// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::collections::HashMap;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;
use std::time::Instant;

use futures::stream::FuturesUnordered;
use futures::Future;
use futures::FutureExt;
use futures::StreamExt;
use pin_project::pin_project;
use tokio::sync::oneshot;
use tokio::time::Sleep;
use tracing::debug;
use tracing::instrument::Instrumented;
use tracing::Instrument;
use tracing::Span;
use uuid::Uuid;

/// [DynamicTaskQueue] is a task queue that runs tasks in parallel. The queue
/// limits how fast tasks can be started and how many tasks can be running at
/// once, without users having to specify exact limits on either. The queue
/// dynamically computes the maximum number of tasks that can be started per
/// second and the maximum queue length based on a number of heuristics:
///
/// - Rate limit failures during task execution
/// - Task completion rate
/// - Ongoing task executions queue length
///
/// The queue is designed to be used with a large number of tasks that are
/// relatively quick to execute (p95 at most at 3 seconds). It is not designed
/// to be used with long running tasks.
///
/// The queue starts out at a rate of 500 tasks per second, and will increase
/// the rate if the tasks are completing fast enough. The queue will
/// automatically slow down if the tasks are starting to queue up for too long.
///
/// The queue is a stream that returns `Result<(), E>` where `E` is the error
/// type of the tasks. The stream will return `Ok(())` when all tasks have
/// completed successfully, or `Err(e)` when any task has failed.
pub struct DynamicTaskQueue<T: RestartableTask> {
  tasks: VecDeque<T>,
  ongoing: FuturesUnordered<T::Fut>,

  starts: VecDeque<std::time::Instant>,
  ends: VecDeque<std::time::Instant>,
  max_starts_per_second: f64,

  backoff: bool,
  sleep_fut: Option<Pin<Box<Sleep>>>,
}

impl<T: RestartableTask> DynamicTaskQueue<T> {
  pub fn new(tasks: Vec<T>) -> Self {
    Self {
      tasks: tasks.into(),
      ongoing: FuturesUnordered::new(),

      starts: VecDeque::new(),
      ends: VecDeque::new(),
      max_starts_per_second: 500.0,

      backoff: false,
      sleep_fut: None,
    }
  }

  /// Return the deadline for when the next task can be started, or [None] if
  /// the next task can be started immediately.
  ///
  /// The deadline is computed to spread the maximum amount of tasks that may be
  /// started per second evenly across the second. For example, if the maximum
  /// number of tasks that may be started per second is 50, and 20 tasks have
  /// already been started in the first half of the second, then the next task
  /// can be started 0.5s / 30 = 0.016s after the last task that was started.
  fn delay_start_until(&mut self) -> Option<Instant> {
    let now = std::time::Instant::now();
    while let Some(start) = self.starts.front() {
      if now.duration_since(*start) > Duration::from_secs(1) {
        self.starts.pop_front();
      } else {
        break;
      }
    }
    let oldest_start = self.starts.front()?;
    let newest_start = self.starts.back().unwrap();

    let left_to_start_this_second = (self.max_starts_per_second as u32)
      .saturating_sub(self.starts.len() as u32);
    let time_left_in_second =
      Duration::from_secs(1) - (newest_start.duration_since(*oldest_start));

    if time_left_in_second.is_zero() {
      None
    } else if left_to_start_this_second == 0 {
      let deadline = *oldest_start + Duration::from_secs(1);
      if deadline < now {
        return None;
      }
      Some(deadline)
    } else {
      let delay = time_left_in_second.checked_div(left_to_start_this_second)?;
      let deadline = *newest_start + delay;
      if deadline < now {
        return None;
      }
      Some(deadline)
    }
  }

  /// The amount of tasks that have ended with an Ok result in the last second.
  fn ends_in_last_second(&mut self, now: Instant) -> usize {
    while let Some(end) = self.ends.front() {
      if now.duration_since(*end) > Duration::from_secs(1) {
        self.ends.pop_front();
      } else {
        break;
      }
    }
    self.ends.len()
  }
}

impl<T: RestartableTask> futures::Stream for DynamicTaskQueue<T> {
  type Item = Result<T::Ok, T::Err>;

  /// Poll the queue for the next task to finish. The queue will automatically
  /// start new tasks as long as the maximum number of tasks that may be started
  /// per second has not been reached.
  ///
  /// The method will return [Poll::Ready(None)] when all tasks have completed
  /// successfully, or [Poll::Ready(Some(Err(e)))] when any task has failed.
  fn poll_next(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> Poll<Option<Self::Item>> {
    let this = &mut *self;
    // Loop until we are blocked on both the sleep future, and the ongoing tasks
    // future all returning [Poll::Pending].
    'outer: loop {
      // If there is a sleep future, poll it and remove it if it is ready. If
      // the sleep future is not ready and we have no ongoing tasks, then we
      // can skip the rest of the loop and return pending immediately.
      if let Some(sleep_fut) = this.sleep_fut.as_mut() {
        if sleep_fut.poll_unpin(cx).is_ready() {
          this.sleep_fut = None;
          this.backoff = false;
        } else if this.ongoing.is_empty() {
          break Poll::Pending;
        }
      }

      // If we are not blocked on the sleep future, attempt to start new tasks
      // until we are blocked on the sleep future.
      while this.sleep_fut.is_none() {
        // Schedule a sleep if we are at the maximum number of tasks that may
        // be started per second.
        if let Some(deadline) = this.delay_start_until() {
          this.sleep_fut =
            Some(Box::pin(tokio::time::sleep_until(deadline.into())));
          // We do not return a poll result, but go around the loop again to
          // ensure the sleep future is polled at least once so that the waker
          // is registered.
          continue 'outer;
        }

        // If there are more tasks to start, start one. Otherwise, we can exit
        // this loop.
        if let Some(task) = this.tasks.pop_front() {
          this.ongoing.push(task.run());
          this.starts.push_back(std::time::Instant::now());
        } else {
          break;
        }
      }

      // Poll the ongoing futures. If any future is ready, deal with the result:
      match this.ongoing.poll_next_unpin(cx) {
        Poll::Ready(Some(RestartableTaskResult::Ok(d))) => {
          let now = std::time::Instant::now();
          // If the future was successful, record the end time.
          this.ends.push_back(now);
          // We now check whether we can increase the maximum max task start
          // rate. To increase it, the number of tasks that actually ended in
          // the last second must be at least 97% of the maximum number of tasks
          // that may be started per second. If it is, we increase the maximum
          // number of tasks that may be started per second by 5%.
          let minimum_ends_to_advance =
            (this.max_starts_per_second * 0.97).floor() as usize;
          let ends_in_last_second = this.ends_in_last_second(now);
          if ends_in_last_second >= minimum_ends_to_advance {
            let before = this.max_starts_per_second;
            this.max_starts_per_second *= 1.05;
            debug!(target: "DynamicTaskQueue::poll_next", "increasing max_starts_per_second from {} to {}", before, this.max_starts_per_second);
          }
          // If we are starting to queue up tasks, ie the ongoing task queue
          // length is 3x longer than the amount of tasks that ended in the last
          // second, we decrease the maximum number of tasks that may be started
          // per second to the current rate of tasks ending * 0.98. We however
          // only do this if the queue length is 3x longer than the max number
          // of tasks we can start a second - this is to avoid heavy downscaling
          // when the queue is just starting up. This effectively limits queue
          // tasks to take at most 3 seconds.
          if this.ongoing.len() / 3 > this.max_starts_per_second as usize
            && this.ongoing.len() / 3 > ends_in_last_second
          {
            let before = this.max_starts_per_second;
            this.max_starts_per_second = ends_in_last_second as f64 * 0.98;
            debug!(target: "DynamicTaskQueue::poll_next", "decreasing max_starts_per_second from {} to {}", before, this.max_starts_per_second);
          }
          // We return ready because we have finished a task.
          break Poll::Ready(Some(Ok(d)));
        }
        Poll::Ready(Some(RestartableTaskResult::Backoff(task))) => {
          // If the task returned a backoff result, we need to back off. If we
          // are not already processing backing off, we schedule to sleep for 1
          // second and decrease the maximum number of tasks that may be started
          // per second by 50%.
          if !this.backoff {
            debug!(target: "DynamicTaskQueue::poll_next", "backing off");
            this.backoff = true;
            this.sleep_fut =
              Some(Box::pin(tokio::time::sleep(Duration::from_secs(1))));
            this.max_starts_per_second *= 0.5;
          }
          this.tasks.push_back(task);
          // We do not return a poll result, but go around the loop again to
          // ensure the sleep future is polled at least once so that the waker
          // is registered.
        }
        Poll::Ready(Some(RestartableTaskResult::Error(e))) => {
          // If the task returned an error, we return ready with the error.
          break Poll::Ready(Some(Err(e)));
        }
        Poll::Ready(None) => {
          // If the ongoing futures stream is done, we return ready with none
          // only if there are no more tasks to start. If there are more tasks,
          // there must be a sleep future, otherwise tasks would have been
          // started onto the ongoing futures stream and we would not be in this
          // branch.
          if this.tasks.is_empty() {
            break Poll::Ready(None);
          }
          assert!(this.sleep_fut.is_some());
        }
        Poll::Pending => {
          // If the ongoing futures stream is pending, we return pending. When
          // breaking here we have ensured that the sleep future has been polled
          // at least once so that the waker is registered.
          break Poll::Pending;
        }
      }
    }
  }
}

pub enum RestartableTaskResult<S: RestartableTask> {
  /// The queue task completed successfully, and the result can be yielded to
  /// the caller.
  Ok(S::Ok),
  /// This task hit a rate limit, and should be retried after a backoff.
  /// The task is returned so that it can be retried.
  Backoff(S),
  /// This task failed with an error. The error is returned to the caller.
  /// This error is fatal - the queue will not continue to run after this.
  Error(S::Err),
}

pub trait RestartableTask: Sized + Unpin {
  type Ok;
  type Err;
  type Fut: Future<Output = RestartableTaskResult<Self>>;

  fn run(self) -> Self::Fut;
}

type TaskResultSender<T> = oneshot::Sender<
  Result<<T as RestartableTask>::Ok, <T as RestartableTask>::Err>,
>;

/// [DynamicBackgroundTaskQueue] is similar to [DynamicTaskQueue], but it runs
/// tasks in the background, channeling the results back to the inserter, rather
/// than returning the results directly.
pub struct DynamicBackgroundTaskQueue<T: RestartableTask> {
  sender: tokio::sync::mpsc::UnboundedSender<(
    InstrumentedQueueTask<T>,
    TaskResultSender<T>,
  )>,
}

impl<T: RestartableTask> Clone for DynamicBackgroundTaskQueue<T> {
  fn clone(&self) -> Self {
    Self {
      sender: self.sender.clone(),
    }
  }
}

impl<T: RestartableTask> Default for DynamicBackgroundTaskQueue<T>
where
  T: Send + 'static,
  T::Ok: Send + 'static,
  T::Err: Send + 'static,
  T::Fut: Send + 'static,
{
  fn default() -> Self {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(DynamicBackgroundTaskQueueDriveFuture {
      queue: DynamicTaskQueue::new(Vec::new()),
      new_tasks: Some(receiver),
      senders: HashMap::new(),
    });
    Self { sender }
  }
}

impl<T: RestartableTask> DynamicBackgroundTaskQueue<T>
where
  T: Send + 'static,
  T::Ok: Send + 'static,
  T::Err: Send + 'static,
  T::Fut: Send + 'static,
{
  pub fn run(&self, task: T) -> oneshot::Receiver<Result<T::Ok, T::Err>> {
    let (sender, receiver) = oneshot::channel();
    let instrumented_queue_task = InstrumentedQueueTask::new(task);
    self.sender.send((instrumented_queue_task, sender)).unwrap();
    receiver
  }
}

#[pin_project]
pub struct DynamicBackgroundTaskQueueDriveFuture<T: RestartableTask> {
  #[pin]
  queue: DynamicTaskQueue<QueueTaskWithId<T>>,
  new_tasks:
    Option<tokio::sync::mpsc::UnboundedReceiver<(T, TaskResultSender<T>)>>,
  senders: HashMap<Uuid, TaskResultSender<T>>,
}

impl<T: RestartableTask> Future for DynamicBackgroundTaskQueueDriveFuture<T> {
  type Output = ();

  fn poll(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> Poll<Self::Output> {
    let mut this = self.project();
    loop {
      // If there are new tasks, add them to the queue. If the queue is done, we
      // remove the new tasks receiver.
      if let Some(fut) = this.new_tasks {
        match fut.poll_recv(cx) {
          Poll::Ready(Some((task, sender))) => {
            let id = Uuid::new_v4();
            this.queue.tasks.push_back(QueueTaskWithId { id, task });
            this.senders.insert(id, sender);
            continue;
          }
          Poll::Ready(None) => {
            *this.new_tasks = None;
          }
          Poll::Pending => {}
        }
      };

      // Poll the queue for the next task to finish. If the queue is done, we
      // return ready if the new tasks receiver is also done. If it is not, we
      // return pending.
      match futures::Stream::poll_next(this.queue.as_mut(), cx) {
        Poll::Ready(Some(Ok((id, res)))) => {
          let sender = this.senders.remove(&id).unwrap();
          sender.send(Ok(res)).ok(); // if the receiver is dropped, ignore
        }
        Poll::Ready(Some(Err((id, err)))) => {
          let sender = this.senders.remove(&id).unwrap();
          sender.send(Err(err)).ok(); // if the receiver is dropped, ignore
        }
        Poll::Ready(None) => {
          if this.new_tasks.is_none() {
            return Poll::Ready(());
          }
          return Poll::Pending;
        }
        Poll::Pending => return Poll::Pending,
      }
    }
  }
}

struct QueueTaskWithId<T: RestartableTask> {
  id: Uuid,
  task: T,
}

impl<T: RestartableTask> RestartableTask for QueueTaskWithId<T> {
  type Ok = (Uuid, T::Ok);
  type Err = (Uuid, T::Err);
  type Fut = QueueTaskWithIdFuture<T>;

  fn run(self) -> Self::Fut {
    QueueTaskWithIdFuture {
      id: self.id,
      fut: self.task.run(),
    }
  }
}

#[pin_project]
struct QueueTaskWithIdFuture<T: RestartableTask> {
  id: Uuid,
  #[pin]
  fut: T::Fut,
}

impl<T: RestartableTask> Future for QueueTaskWithIdFuture<T> {
  type Output = RestartableTaskResult<QueueTaskWithId<T>>;

  fn poll(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> Poll<Self::Output> {
    let this = self.project();
    this.fut.poll(cx).map(|res| match res {
      RestartableTaskResult::Ok(d) => RestartableTaskResult::Ok((*this.id, d)),
      RestartableTaskResult::Backoff(t) => {
        RestartableTaskResult::Backoff(QueueTaskWithId {
          id: *this.id,
          task: t,
        })
      }
      RestartableTaskResult::Error(e) => {
        RestartableTaskResult::Error((*this.id, e))
      }
    })
  }
}

struct InstrumentedQueueTask<T: RestartableTask> {
  span: Span,
  task: T,
}

impl<T: RestartableTask> InstrumentedQueueTask<T> {
  pub fn new(task: T) -> Self {
    Self {
      span: Span::current(),
      task,
    }
  }
}

impl<T: RestartableTask> RestartableTask for InstrumentedQueueTask<T> {
  type Ok = T::Ok;
  type Err = T::Err;
  type Fut = InstrumentedQueueTaskFuture<T>;

  fn run(self) -> Self::Fut {
    InstrumentedQueueTaskFuture {
      fut: self.task.run().instrument(self.span),
    }
  }
}

#[pin_project]
struct InstrumentedQueueTaskFuture<T: RestartableTask> {
  #[pin]
  fut: Instrumented<T::Fut>,
}

impl<T: RestartableTask> Future for InstrumentedQueueTaskFuture<T> {
  type Output = RestartableTaskResult<InstrumentedQueueTask<T>>;

  fn poll(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> Poll<Self::Output> {
    let this = self.project();
    let span = this.fut.span().clone();
    this.fut.poll(cx).map(|res| match res {
      RestartableTaskResult::Ok(d) => RestartableTaskResult::Ok(d),
      RestartableTaskResult::Backoff(t) => {
        RestartableTaskResult::Backoff(InstrumentedQueueTask { span, task: t })
      }
      RestartableTaskResult::Error(e) => RestartableTaskResult::Error(e),
    })
  }
}

import { IS_BROWSER } from "$fresh/runtime.ts";
import { useState,useEffect  } from "preact/hooks";
import { api, path } from "../utils/api.ts";
import { PublishingTask } from "../utils/api_types.ts";

export default function PollPublishingTask(
  { date, packageName }: { date: string; packageName: string },
) {
  if (!IS_BROWSER) {
    return null;
  }

  const [message, setMessage] = useState<
    undefined | { errored: false } | {
      errored: true;
      code: string;
      message: string;
      task: string;
    }
  >(undefined);

  useEffect(() => {
    setTimeout(() => {
      setMessage({ errored: false });
    }, 10_000);

    const [scope, name] = packageName.split("/");
    poll(new Date(date), scope.substring(1), name).then((error) => {
      if (error == null) {
        window.location.pathname = `/${packageName}`;
      } else {
        setMessage({
          errored: true,
          code: error.code,
          message: error.message,
          task: error.id,
        });
      }
    });
  }, []);

  return (
    <div>
      {message && (message.errored
        ? (
          <div class="space-y-3">
            <div className="bg-red-100 rounded border-2 border-red-200 py-1.5 px-3 flex justify-between gap-3">
              <div className="space-y-1.5">
                <div className="font-bold text-xl">
                  Publishing errored: {message.code}
                </div>
                <div>
                  {message.message}
                </div>
              </div>
            </div>

            <div>
              For more information, go to{"  "}
              <a href={`/status/${message.task}`}>the publishing task page</a>
            </div>
          </div>
        )
        : <div>Waiting a long time? Check your terminal for more detailed status.</div>)}
    </div>
  );
}

async function poll(
  date: Date,
  scope: string,
  name: string,
): Promise<null | { code: string; message: string; id: string }> {
  await new Promise((res) => setTimeout(res, 2000));

  const res = await api.get<PublishingTask[]>(
    path`/scopes/${scope}/packages/${name}/publishing_tasks`,
  );

  if (res.ok) {
    const task = res.data.find((task) => new Date(task.createdAt) > date);

    if (task) {
      if (task.status === "success") {
        return null;
      } else if (task.status === "failure") {
        return {
          code: task.error!.code,
          message: task.error!.message,
          id: task.id,
        };
      }
    }
  }

  return await poll(date, scope, name);
}

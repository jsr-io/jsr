-- just update the status enum.
CREATE TYPE task_status2 AS ENUM ('pending', 'processing', 'success', 'failure');
ALTER TABLE publishing_tasks ADD COLUMN status_new task_status2 NOT NULL DEFAULT 'pending';
UPDATE publishing_tasks SET status_new = status::text::task_status2;
ALTER TABLE publishing_tasks DROP COLUMN status;
ALTER TABLE publishing_tasks RENAME COLUMN status_new TO status;
DROP TYPE task_status;

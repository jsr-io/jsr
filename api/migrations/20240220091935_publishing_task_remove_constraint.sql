alter table publishing_tasks
    drop constraint publishing_tasks_package_scope_fkey;

alter table publishing_tasks
    drop constraint publishing_tasks_package_scope_package_name_fkey;

alter table package_version_dependencies
    drop constraint package_version_dependencies_package_scope_package_name_fkey;

alter table package_version_dependencies
    add foreign key (package_scope, package_name) references packages ON UPDATE CASCADE ON DELETE CASCADE;


alter table package_version_dependencies
    drop constraint package_version_dependencies_package_scope_package_name_pa_fkey;

alter table package_version_dependencies
    add constraint package_version_dependencies_package_scope_package_name_pa_fkey
        foreign key (package_scope, package_name, package_version) references package_versions ON UPDATE CASCADE ON DELETE CASCADE;

alter table package_files
    drop constraint package_files_scope_name_version_fkey;

alter table package_files
    add foreign key (scope, name, version) references package_versions
        ON UPDATE CASCADE ON DELETE CASCADE;

alter table npm_tarballs
    drop constraint npm_tarballs_scope_name_version_fkey;

alter table npm_tarballs
    add foreign key (scope, name, version) references package_versions
        on UPDATE CASCADE ON DELETE CASCADE;

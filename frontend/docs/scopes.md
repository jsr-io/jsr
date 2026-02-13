---
title: Scopes
description: Scopes are groups that jointly administer a set of packages. Learn how to create and manage scopes.
---

On JSR, all packages are contained within a scope. A scope is a group that
jointly administers a set of packages. Scopes have no single owner, and are
instead managed by one or more admins.

Scopes on JSR are similar to npm organizations. Scopes are prefixed with an `@`
symbol. Scope names must be between 2 and 20 characters long, and can only
contain lowercase letters, numbers, and hyphens. They cannot start with a
hyphen. Scope names are globally unique - no two scopes can have the same name.

You can create a scope at [jsr.io/new](/new).

## Members

Scopes can have multiple members. Members can administer the scope and publish
packages.

### Roles

Members can have one of two roles: admin or member.

| Permission                 | Member | Admin |
| -------------------------- | ------ | ----- |
| Create packages            | ✅     | ✅    |
| Delete packages            | ❌     | ✅    |
| Publish package versions   | ✅     | ✅    |
| Yank package versions      | ❌     | ✅    |
| Update package description | ✅     | ✅    |
| Update package GitHub repo | ❌     | ✅    |
| Invite members             | ❌     | ✅    |
| Change member roles        | ❌     | ✅    |
| Remove members             | ❌     | ✅    |
| Delete scope               | ❌     | ✅    |

The user that creates a scope is automatically an admin of that scope. Admins
can invite other users to join the scope, and can change the role of other
members. Admins can also delete the scope.

### Inviting members

Admins can invite other users to join the scope. To invite a user, head to the
"Members" tab of your scope, enter the GitHub username of the user you want to
invite, and click "Invite". The user will receive an email inviting them to join
the scope. The user must have a JSR account before you can invite them to join
your scope. If they do not have a JSR account yet, ask them to create one at
[jsr.io](/) before inviting them.

When a user is invited, they will receive an email inviting them to join the
scope. The email will contain a link to accept the invitation. If the user
accepts the invitation, they will be added to the scope as a member. The user
can also see all pending invitations on their
[account invitations page](/account/invites).

### Changing member roles

Admins can change the role of other members. To change the role of a member,
head to the "Members" tab of your scope, click the dropdown next to the member
you want to change the role of, and select the new role. The new role will take
effect immediately. Note that you cannot demote the last admin of a scope to a
member. If you want to demote the last admin of a scope to a member, you must
first promote another member to an admin. A scope must always have at least one
admin.

### Removing members

Admins can remove members from the scope. To remove a member, head to the
"Members" tab of your scope, click the dropdown next to the member you want to
remove, and select "Remove". The member will be removed from the scope
immediately. The user will no longer be able to administer or publish to the
scope. If the member is the last admin of the scope, you must first promote
another member to an admin before you can remove the last admin.

### Leaving a scope

You can leave a scope at any time. To leave a scope, head to the "Members" tab
of the scope, and click "Leave". You will be removed from the scope immediately.
You will no longer be able to access to administer or publish to the scope. If
you are the last admin of the scope, you must first promote another member to an
admin before you can leave the scope.

If you are the last member of a scope, you cannot leave the scope. You can
[delete the scope](#deleting-a-scope) instead.

## Deleting a scope

Scopes can only be deleted if they have no packages. If you want to delete a
scope, you must first delete all packages in that scope.
[Learn more about package deletion.](/docs/packages#deleting-a-package)

Scopes can be deleted by scope admins from the scope settings page.

## GitHub Actions publishing security

If you link a package in your scope to a GitHub repository, you can publish
packages from GitHub Actions without having to configure any secrets or
authentication.
[Learn more about publishing from GitHub Actions.](/docs/publishing-packages#publishing-from-github-actions)

Firstly, publishing is permitted only if the GitHub Actions workflow runs in the
GitHub repository that is linked to the package on JSR.

As a scope admin you can additionally restrict publishing to be permitted only
if the user that triggered the GitHub Actions workflow is a member of this scope
on JSR. This option is enabled by default.

You can disable this option in the scope settings page to allow publishing from
any GitHub Actions workflow in the linked GitHub repository, regardless of the
user that triggered the workflow.

## Requiring publishing from CI

As a scope admin you can require that all package versions are published from a
CI environment (GitHub Actions). Enabling this option will prevent users from
publishing package versions from their local development environment. All
package versions must be published with an OIDC token from a CI environment.

You can enable this option in the scope settings page.

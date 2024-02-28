// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import { useEffect } from "preact/hooks";
import { FullUser, Package } from "../utils/api_types.ts";
import { api, path } from "../utils/api.ts";
import { getOrInsertItem } from "../utils/client_cache.ts";
import { GitHubRepoInput } from "../components/GitHubRepoInput.tsx";

export function GitHubActionsLink(
  { pkg, user }: { pkg: Package; user?: FullUser },
) {
  const linking = useSignal(false);
  const owner = useSignal("");
  const repo = useSignal("");
  const error = useSignal("");

  useEffect(() => {
    if (user) {
      getOrInsertItem(
        `gh-login-${user.githubId}`,
        () =>
          fetch(`https://api.github.com/user/${user.githubId}`, {
            headers: {
              "Content-Type": "application/json",
            },
          })
            .then((r) => r.json())
            .then((data) => {
              return data.login;
            }),
      ).then((login) => {
        if (owner.value == "") owner.value = login;
      })
        .catch(console.error);
    }
  });

  async function onSubmit(e: Event) {
    e.preventDefault();
    linking.value = true;

    const resp = await api.patch<Package>(
      path`/scopes/${pkg.scope}/packages/${pkg.name}`,
      { githubRepository: { owner: owner.value, name: repo.value } },
    );
    if (resp.ok) {
      location.reload();
    } else {
      console.error(resp);
      error.value = resp.message;
    }

    linking.value = false;
  }

  return (
    <>
      <form class="mt-2 flex gap-4 items-center flex-wrap" onSubmit={onSubmit}>
        <label for="gh-repo-input" class="sr-only">GitHub Repository</label>
        <GitHubRepoInput
          id="gh-repo-input"
          owner={owner}
          repo={repo}
          error={error}
          required={true}
          disabled={linking}
        />
        <button class="button-primary" disabled={linking}>
          {linking.value ? "Linking..." : "Link"}
        </button>
      </form>
      {error.value && <p class="text-sm text-yellow-600">{error}</p>}
    </>
  );
}

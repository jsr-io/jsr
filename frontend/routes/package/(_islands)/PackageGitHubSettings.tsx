// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import { GitHubRepoInput } from "../../../components/GitHubRepoInput.tsx";
import { api, path } from "../../../utils/api.ts";
import { GithubRepository } from "../../../utils/api_types.ts";
import { JSX } from "preact";

export interface PackageGitHubSettingsProps {
  scope: string;
  package: string;
  repo: GithubRepository | null;
}

export function PackageGitHubSettings(
  props: PackageGitHubSettingsProps,
) {
  const processing = useSignal(false);

  const originalOwner = props.repo?.owner ?? "";
  const originalRepo = props.repo?.name ?? "";
  const owner = useSignal<string>(originalOwner);
  const repo = useSignal<string>(originalRepo);
  const error = useSignal<string>("");

  async function onSubmit(e: JSX.TargetedEvent<HTMLFormElement>) {
    e.preventDefault();

    const data = new FormData(e.currentTarget);

    const rawOwner = data.get("owner");
    const rawRepo = data.get("repo");
    owner.value = rawOwner ? String(rawOwner) : "";
    repo.value = rawRepo ? String(rawRepo) : "";

    processing.value = true;
    const res = await api.patch(
      path`/scopes/${props.scope}/packages/${props.package}`,
      { githubRepository: { owner: owner.value, name: repo.value } },
    );
    processing.value = false;
    if (!res.ok) {
      console.error(res);
      error.value = res.message;
      return;
    }
    error.value = "";
    location.reload();
  }

  return (
    <div class="space-y-4">
      <form method="POST" onSubmit={onSubmit} class="flex flex-wrap gap-4">
        <GitHubRepoInput
          id="gh-repo-input"
          error={error}
          owner={owner}
          repo={repo}
          required={false}
          disabled={processing}
        />

        <button
          class="button-primary"
          type="submit"
          name="action"
          value="updateRepo"
          disabled={processing.value ||
            !(owner.value !== originalOwner || repo.value !== originalRepo)}
        >
          Save
        </button>
      </form>
      {props.repo && (
        <form method="POST">
          <button
            class="button-danger"
            type="submit"
            name="action"
            value="unlinkRepo"
            disabled={processing.value || !props.repo}
          >
            Unlink repository
          </button>
        </form>
      )}
      {error && <p class="text-red-600 mt-2">{error}</p>}
    </div>
  );
}

// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Signal } from "@preact/signals";
import { useRef } from "preact/hooks";
import TbBrandGithub from "tb-icons/TbBrandGithub";

export interface GithubRepoInputProps {
  id: string;
  owner: Signal<string>;
  repo: Signal<string>;
  error: Signal<string>;
  required: boolean;
  disabled: Signal<boolean>;
}

export function GitHubRepoInput(
  { id, owner, repo, error, required, disabled }: GithubRepoInputProps,
) {
  const repoRef = useRef<HTMLInputElement>(null);

  function onPaste(e: ClipboardEvent) {
    const data = e.clipboardData?.getData("text");
    if (typeof data === "string") {
      // Case: https://github.com/preactjs/preact
      if (data.startsWith("https://github.com/")) {
        try {
          const url = new URL(data);
          const parts = url.pathname.slice(1).split("/");
          if (parts.length === 2) {
            e.preventDefault();
            owner.value = parts[0];
            repo.value = parts[1];
          }
        } catch (_err) {
          // Ignore, likely not a valid URL
        }
      }

      // Case: preactjs/preact
      const parts = data.split("/");
      if (parts.length === 2) {
        e.preventDefault();
        owner.value = parts[0];
        repo.value = parts[1];
      }
    }
  }

  return (
    <div class="flex items-center w-full md:w-88 rounded-md text-primary shadow-xs pl-3 py-[2px] pr-[2px] sm:text-sm sm:leading-6 bg-white dark:bg-jsr-gray-900 input-container">
      <span class="block">
        <TbBrandGithub class="!size-5" />
      </span>
      <input
        id={id}
        class="py-1.5 pr-1 pl-2 grow w-0 input"
        type="text"
        name="owner"
        placeholder="octocat"
        value={owner}
        required={required}
        onPaste={onPaste}
        onKeyUp={(e) => {
          // Focus to next input when the user types a "/"
          if (e.key === "/") {
            e.preventDefault();
            const value = e.currentTarget.value.slice(0, -1);
            e.currentTarget.value = value;
            owner.value = value;
            setTimeout(() => {
              repoRef.current?.focus();
            }, 0);
          }
        }}
        onInput={(e) => {
          owner.value = e.currentTarget.value;
          error.value = "";
        }}
        disabled={disabled}
      />
      <span class="block text-tertiary">/</span>
      <input
        ref={repoRef}
        class="py-1.5 pr-4 pl-1 grow w-0 input rounded-md"
        type="text"
        name="repo"
        placeholder="Spoon-Knife"
        value={repo}
        required={required}
        onPaste={onPaste}
        onInput={(e) => {
          repo.value = e.currentTarget.value;
          error.value = "";
        }}
        disabled={disabled}
      />
    </div>
  );
}

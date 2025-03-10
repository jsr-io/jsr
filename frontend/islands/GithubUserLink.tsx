// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import TbBrandGithub from "tb-icons/TbBrandGithub";
import { useEffect } from "preact/hooks";
import { User } from "../utils/api_types.ts";
import { cachedGitHubLogin } from "../utils/github.ts";

export function GitHubUserLink({ user }: { user?: User }) {
  const login = useSignal("");
  const error = useSignal(false);

  useEffect(() => {
    if (user) {
      cachedGitHubLogin(user)
        .then((login_) => {
          login.value = login_;
        })
        .catch((error_) => {
          console.error(error_);

          error.value = true;
        });
    }
  });

  if (error.value) {
    return (
      <span class="italic text-[0.625rem]">Could not load GitHub username</span>
    );
  }

  return login.value == ""
    ? <span className="text-gray-600">loading...</span>
    : (
      <a
        class="link inline-flex justify-center items-center gap-1"
        href={"https://github.com/" + login.value}
      >
        <TbBrandGithub class="size-4" aria-hidden />
        GitHub
      </a>
    );
}

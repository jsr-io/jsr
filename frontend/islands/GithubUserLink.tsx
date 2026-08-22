// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import TbBrandGithub from "tb-icons/TbBrandGithub";
import { useEffect } from "preact/hooks";
import { User } from "../utils/api_types.ts";
import { cachedGitHubLogin } from "../utils/github.ts";

export function GitHubUserLink({ user }: { user: User }) {
  const login = useSignal("");
  const error = useSignal(false);

  useEffect(() => {
    if (user.githubId !== null) {
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

  const icon = (
    <TbBrandGithub
      class="size-6 text-white rounded-full p-1 bg-jsr-gray-900"
      aria-hidden
    />
  );

  if (user.githubId === null) {
    return (
      <span className="text-tertiary text-sm inline-flex justify-center items-center gap-1">
        {icon}
        account not linked
      </span>
    );
  } else if (error.value) {
    return (
      <span className="text-tertiary text-sm inline-flex justify-center items-center gap-1">
        {icon}
        unavailable
      </span>
    );
  } else if (login.value == "") {
    return (
      <span className="text-tertiary text-sm inline-flex justify-center items-center gap-1">
        {icon}
        loading...
      </span>
    );
  } else {
    return (
      <a
        class="link inline-flex justify-center items-center gap-1"
        href={"https://github.com/" + login.value}
      >
        {icon}
        {login.value}
      </a>
    );
  }
}

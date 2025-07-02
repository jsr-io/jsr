// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import TbBrandGitlab from "tb-icons/TbBrandGitlab";
import { useEffect } from "preact/hooks";
import { User } from "../utils/api_types.ts";
import { cachedGitLabUsername } from "../utils/gitlab.ts";

export function GitLabUserLink({ user }: { user: User }) {
  const login = useSignal("");
  const error = useSignal(false);

  useEffect(() => {
    if (user.gitlabId !== null) {
      cachedGitLabUsername(user)
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
    <TbBrandGitlab
      class="size-6 text-white rounded-full p-1 bg-jsr-gray-900"
      aria-hidden
    />
  );

  if (user.gitlabId === null) {
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
        href={"https://gitlab.com/" + login.value}
      >
        {icon}
        {login.value}
      </a>
    );
  }
}

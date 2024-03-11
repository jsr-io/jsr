// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import { useEffect } from "preact/hooks";
import { Package, User } from "../utils/api_types.ts";
import { api, path } from "../utils/api.ts";
import { GitHubRepoInput } from "../components/GitHubRepoInput.tsx";
import { cachedGitHubLogin } from "../utils/github.ts";

export function GitHubUserLink({ user }: { user?: User }) {
  const login = useSignal("");

  useEffect(() => {
    if (user) {
      cachedGitHubLogin(user)
        .then((login_) => {
          login.value = login_;
        })
        .catch(console.error);
    }
  });

  return login.value == ""
    ? <span>loading...</span>
    : <a class="link" href={"https://github.com/" + login.value}>GitHub</a>;
}

// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { User } from "./api_types.ts";
import { getOrInsertItem } from "./client_cache.ts";

export async function cachedGitHubLogin(user: User): Promise<string> {
  return await getOrInsertItem(
    `gh-login-${user.githubId}`,
    () => {
      const fetchGithubUser = async () => {
        const response = await fetch(
          `https://api.github.com/user/${user.githubId}`,
          {
            headers: {
              "Content-Type": "application/json",
            },
          },
        );

        const data = await response.json();

        if (!data.login) {
          await new Promise((resolve) => setTimeout(resolve, 100));
          return fetchGithubUser();
        }
        return data.login;
      };

      return fetchGithubUser();
    },
  );
}

// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { User } from "./api_types.ts";
import { getOrInsertItem } from "./client_cache.ts";

export async function cachedGitHubLogin(user: User): Promise<string> {
  return await getOrInsertItem(
    `gh-login-${user.githubId}`,
    () => {
      const MAX_RETRIES = 3;
      const fetchGithubUser = async (retryCount = 0) => {
        const response = await fetch(
          `https://api.github.com/user/${user.githubId}`,
          {
            headers: {
              "Content-Type": "application/json",
            },
          },
        );

        if (
          response.status === 403 &&
          response.headers.get("x-ratelimit-remaining") === "0"
        ) {
          throw new Error("Github API rate limit exceeded");
        }

        const data = await response.json();

        if (!data.login) {
          if (retryCount >= MAX_RETRIES) {
            throw new Error(
              "Failed to fetch GitHub login after maximum retries",
            );
          }

          await new Promise((resolve) => setTimeout(resolve, 100));
          return fetchGithubUser(retryCount + 1);
        }
        return data.login;
      };

      return fetchGithubUser();
    },
  );
}

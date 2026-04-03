// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { User } from "./api_types.ts";
import { getOrInsertItem } from "./client_cache.ts";

export async function cachedGitLabUsername(user: User): Promise<string> {
  return await getOrInsertItem(
    `gl-username-${user.gitlabId}`,
    () => {
      const MAX_RETRIES = 3;
      const fetchGitlabUser = async (retryCount = 0) => {
        const response = await fetch(
          `https://gitlab.com/api/v4/users/${user.gitlabId}`,
          {
            headers: {
              "Content-Type": "application/json",
            },
          },
        );

        if (
          response.status === 403 &&
          response.headers.get("RateLimit-Remaining") === "0"
        ) {
          throw new Error("GitLab API rate limit exceeded");
        }

        const data = await response.json();

        if (!data.username) {
          if (retryCount >= MAX_RETRIES) {
            throw new Error(
              "Failed to fetch GitLab username after maximum retries",
            );
          }

          await new Promise((resolve) => setTimeout(resolve, 100));
          return fetchGitlabUser(retryCount + 1);
        }
        return data.username;
      };

      return fetchGitlabUser();
    },
  );
}

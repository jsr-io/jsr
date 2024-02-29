import { User } from "./api_types.ts";
import { getOrInsertItem } from "./client_cache.ts";

export async function cachedGitHubLogin(user: User): Promise<string> {
  return await getOrInsertItem(
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
  );
}

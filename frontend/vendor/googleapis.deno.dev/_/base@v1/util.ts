// Get an env var, but return "undefined" if it doesn't exist or access is
// denied.
export function getEnv(name: string): string | undefined {
  try {
    return Deno.env.get(name);
  } catch (err) {
    if (err instanceof Deno.errors.PermissionDenied) {
      return undefined;
    }
    throw err;
  }
}

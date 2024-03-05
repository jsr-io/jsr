export const JSR_URL: string = Deno.env.get("JSR_URL")!;
export const JSR_API_URL: string = Deno.env.get("JSR_API_URL")!;

if (!JSR_URL) {
  throw new Error("JSR_URL is not set in the environment");
}

if (!JSR_API_URL) {
  throw new Error("JSR_API_URL is not set in the environment");
}

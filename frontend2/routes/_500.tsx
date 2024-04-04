// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export default function Error({ error }: { error: unknown }) {
  if (
    typeof error === "object" && error !== null && "ok" in error &&
    error.ok === false && "status" in error && "code" in error &&
    "message" in error &&
    "traceId" in error
  ) {
    return (
      <div class="border-t-8 border-red-600 p-4 bg-gray-50">
        <h1 class="text-2xl font-semibold">{error.status} - {error.code}</h1>
        <p class="mt-4">{error.message as string}</p>
        <p class="mt-4 text-sm">Need help? Contact support@deno.com</p>
        <div class="flex gap-4 mt-1">
          <div class="bg-gray-200 py-1 px-2 font-bold text-sm inline-block">
            x-deno-ray:{" "}
            <span class="font-mono">{error.traceId as "string"}</span>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div>
      <h1>Error</h1>
      <pre>{JSON.stringify(error, null, 2)}</pre>
    </div>
  );
}

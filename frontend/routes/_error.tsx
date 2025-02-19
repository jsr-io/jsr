// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError } from "fresh";
import { APIResponseError } from "../utils/api.ts";
import { ErrorDisplay } from "../components/ErrorDisplay.tsx";

export default function Error({ error }: { error: unknown }) {
  if (error instanceof HttpError) {
    if (error.status === 404 && error.message === "Not Found") {
      error.message = "Couldn't find what you're looking for.";
    }
    return (
      <>
        <style
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{
            __html: `#fresh-error-overlay { display: none !important; }`,
          }}
        />
        <div class="w-full overflow-x-hidden relative flex justify-between flex-col flex-wrap">
          <div class="flex-top">
            <header class="text-center px-8 py-[15vh]">
              <h1 class="font-extrabold text-5xl leading-10 tracking-tight text-gray-900">
                {error.status}
              </h1>
              <h2 class="mt-4 sm:mt-5 font-light text-2xl text-center leading-tight text-gray-900">
                {error.message}
              </h2>
              {error.status === 404 && (
                <a class="button-primary mt-4" href="/">
                  Get back to safety
                </a>
              )}
            </header>
          </div>
        </div>
      </>
    );
  }

  if (
    typeof error === "object" && error !== null && "ok" in error &&
    error.ok === false && "status" in error && "code" in error &&
    "message" in error &&
    "traceId" in error
  ) {
    return (
      <>
        <style
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{
            __html: `#fresh-error-overlay { display: none !important; }`,
          }}
        />
        <ErrorDisplay error={error as APIResponseError} />
      </>
    );
  }

  return (
    <div>
      <h1>Error</h1>
      <pre>{JSON.stringify(error, null, 2)}</pre>
    </div>
  );
}

// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, PageProps } from "fresh";
import { APIError } from "../utils/api.ts";
import {
  ErrorDisplay,
  UnknownErrorDisplay,
} from "../components/ErrorDisplay.tsx";

const HIDE_ERROR_OVERLAY_STYLE =
  `#fresh-error-overlay { display: none !important; }`;

export default function Error({ url, error }: PageProps) {
  if (error instanceof HttpError) {
    if (error.status === 404 && error.message === "Not Found") {
      error.message = "Couldn't find what you're looking for.";
    }
    return (
      <>
        <style
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{ __html: HIDE_ERROR_OVERLAY_STYLE }}
        />
        <div class="w-full overflow-x-hidden relative flex justify-between flex-col flex-wrap">
          <div class="flex-top">
            <header class="text-center px-8 py-[15vh]">
              <h1 class="font-extrabold text-5xl leading-10 tracking-tight text-primary">
                {error.status}
              </h1>
              <h2 class="mt-4 sm:mt-5 font-light text-2xl text-center leading-tight text-primary">
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

  if (error instanceof APIError) {
    return (
      <>
        <style
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{ __html: HIDE_ERROR_OVERLAY_STYLE }}
        />
        <ErrorDisplay error={error.response} url={url} />
      </>
    );
  }

  const formatted = Deno.inspect(error);
  return (
    <>
      <style
        // deno-lint-ignore react-no-danger
        dangerouslySetInnerHTML={{ __html: HIDE_ERROR_OVERLAY_STYLE }}
      />

      <UnknownErrorDisplay
        title="An unexpected error occurred"
        message={formatted}
        url={url}
        formatted={formatted}
      />
    </>
  );
}

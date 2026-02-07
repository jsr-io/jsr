// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, PageProps } from "fresh";
import { APIError } from "../utils/api.ts";
import { ErrorDisplay } from "../components/ErrorDisplay.tsx";

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
    const ghUrl = new URL("https://github.com/jsr-io/jsr/issues/new");
    ghUrl.searchParams.append(
      "body",
      `## url:
${url.toString()}

## Error:
\`\`\`json
${JSON.stringify(error.response, null, 2)}
\`\`\`

## Additional context:
    `,
    );
    return (
      <>
        <style
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{ __html: HIDE_ERROR_OVERLAY_STYLE }}
        />
        <ErrorDisplay error={error.response} />
        <a class="button-primary mt-4" href={ghUrl.toString()} target="_blank">
          Open an issue on GitHub
        </a>
      </>
    );
  }

  const ghUrl = new URL("https://github.com/jsr-io/jsr/issues/new");
  ghUrl.searchParams.append(
    "body",
    `## url:
${url.toString()}

## Error:
\`\`\`
${Deno.inspect(error)}
\`\`\`

## Additional context:
    `,
  );
  return (
    <div>
      <h1>Error</h1>
      <pre>{Deno.inspect(error)}</pre>
      <a class="button-primary mt-4" href={ghUrl.toString()} target="_blank">
        Open an issue on GitHub
      </a>
    </div>
  );
}

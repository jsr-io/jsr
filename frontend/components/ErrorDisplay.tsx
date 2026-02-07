// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { APIResponseError } from "../utils/api.ts";
import type { ComponentChildren } from "preact";

export function ErrorDisplay(
  { error, url }: { error: APIResponseError; url: URL },
) {
  return (
    <UnknownErrorDisplay
      title={`${error.status} - ${error.code}`}
      message={error.message}
      url={url}
      formatted={JSON.stringify(error, null, 2)}
      formattedIsJson
    >
      <div class="flex gap-4 mt-4">
        <div class="bg-jsr-gray-200 dark:bg-jsr-gray-700 py-1 px-2 font-bold text-sm inline-block">
          x-deno-ray: <span class="font-mono">{error.traceId as "string"}</span>
        </div>
      </div>
    </UnknownErrorDisplay>
  );
}

export function UnknownErrorDisplay({
  title,
  message,
  url,
  formatted,
  formattedIsJson,
  children,
}: {
  title: string;
  message: string;
  url: URL;
  formatted: unknown;
  formattedIsJson?: boolean;
  children?: ComponentChildren;
}) {
  const ghUrl = new URL("https://github.com/jsr-io/jsr/issues/new");
  ghUrl.searchParams.append(
    "body",
    `## url:
${url.toString()}

## Error:
\`\`\`${formattedIsJson ? "json" : "text"}
${formatted}
\`\`\`

## Additional context:
    `,
  );

  return (
    <div class="border-t-8 border-red-600 p-4 bg-jsr-gray-50 dark:bg-jsr-gray-900">
      <h1 class="text-2xl font-semibold">{title}</h1>
      <p class="mt-4">{message}</p>
      <p class="mt-4 text-sm">Need help?</p>
      <div class="mt-4 flex items-center">
        <a
          class="button-sm button-primary"
          href={ghUrl.toString()}
          target="_blank"
        >
          Open an issue on GitHub
        </a>

        <div class="ml-4 mr-3">
          or
        </div>

        <a
          class="button-sm button-primary"
          href="mailto:help@jsr.io"
          target="_blank"
        >
          Contact help@jsr.io
        </a>
      </div>

      {children}
    </div>
  );
}

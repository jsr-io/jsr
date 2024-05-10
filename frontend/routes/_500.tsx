import { APIResponseError } from "../utils/api.ts";
import { ErrorDisplay } from "../components/ErrorDisplay.tsx";

// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export default function Error({ error }: { error: unknown }) {
  if (
    typeof error === "object" && error !== null && "ok" in error &&
    error.ok === false && "status" in error && "code" in error &&
    "message" in error &&
    "traceId" in error
  ) {
    return <ErrorDisplay error={error as APIResponseError} />;
  }

  return (
    <div>
      <h1>Error</h1>
      <pre>{JSON.stringify(error, null, 2)}</pre>
    </div>
  );
}

// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect, useId, useRef } from "preact/hooks";
import { useSignal } from "@preact/signals";
import TbLoader2 from "tb-icons/TbLoader2";
import { api, path } from "../../../utils/api.ts";
import { PackageScore } from "../../../utils/api_types.ts";

export function RecomputeVersionMeta(
  { scope, pkg }: { scope: string; pkg: string },
) {
  const open = useSignal(false);
  const submitting = useSignal(false);
  const version = useSignal("");
  const result = useSignal<string | null>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const ref = useRef<HTMLFormElement>(null);

  useEffect(() => {
    function outsideClick(e: Event) {
      if (
        (ref.current && !ref.current.contains(e.target as Element)) &&
        (buttonRef.current && !buttonRef.current.contains(e.target as Element))
      ) {
        open.value = false;
      }
    }
    document.addEventListener("click", outsideClick);
    return () => document.removeEventListener("click", outsideClick);
  }, []);

  const prefix = useId();

  return (
    <div class="select-none text-left">
      <button
        ref={buttonRef}
        id={`${prefix}-recompute-modal`}
        class="button-primary"
        type="button"
        onClick={() => open.value = !open.value}
        aria-expanded={open.value ? "true" : "false"}
      >
        recompute
      </button>
      <div
        class={`fixed top-0 right-0 w-screen h-screen bg-gray-300/40 dark:bg-jsr-gray-950/70 z-80 flex justify-center items-center overflow-hidden ${
          open.value ? "opacity-100" : "opacity-0 pointer-events-none"
        } transition`}
        aria-labelledby={`${prefix}-recompute-modal`}
        role="region"
        style="--tw-shadow-color: rgba(156,163,175,0.2);"
      >
        <form
          ref={ref}
          class={`space-y-3 z-90 rounded border-1.5 border-current dark:border-cyan-700 bg-white dark:bg-jsr-gray-950 shadow min-w-96 max-w-[95vw] max-h-[95vh] px-6 py-4 ${
            open.value ? "translate-y-0" : "translate-y-5"
          } transition`}
          style="--tw-shadow-color: rgba(156,163,175,0.2);"
          onSubmit={(e) => {
            e.preventDefault();
            submitting.value = true;
            result.value = null;
            api.post<PackageScore>(
              path`/admin/packages/${scope}/${pkg}/${version.value}/recompute_meta`,
              null,
            ).then((res) => {
              submitting.value = false;
              result.value = res.ok
                ? `done — recomputed score: ${res.data.total}%`
                : `${res.code}: ${res.message}`;
            });
          }}
        >
          <h2 class="text-lg font-semibold text-primary">
            Recompute meta of @{scope}/{pkg}
          </h2>
          <p class="text-sm text-secondary">
            Re-runs the analysis on the stored module files of a version and
            replaces its score meta.
          </p>
          <label class="block space-y-1.5 text-sm text-secondary">
            version
            <input
              name="version"
              required
              class="w-full block px-2 py-1.5 input-container input"
              value={version.value}
              onChange={(event) => {
                version.value = event.currentTarget.value;
              }}
            />
          </label>
          {result.value && <p class="text-sm">{result.value}</p>}
          <button type="submit" class="button-primary" disabled={submitting}>
            {submitting.value && <TbLoader2 class="animate-spin size-4" />}
            recompute
          </button>
        </form>
      </div>
    </div>
  );
}

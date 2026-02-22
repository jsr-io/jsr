// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect, useId, useRef } from "preact/hooks";
import { TbLoader2 } from "tb-icons";
import { api, APIPath } from "../../../utils/api.ts";
import { useSignal } from "@preact/signals";

interface FieldBase {
  name: string;
  label: string;
  type: string;
  required?: boolean;
}

interface SelectField extends FieldBase {
  type: "select";
  values: string[];
  value?: string;
}

interface BooleanField extends FieldBase {
  type: "boolean";
  value?: boolean;
}

interface NumberField extends FieldBase {
  type: "number";
  value?: number;
}

interface TextField extends FieldBase {
  type: "textarea" | "input";
  value?: string;
}

type Field = SelectField | BooleanField | NumberField | TextField;

const BASE_INPUT_STYLING = "w-full block px-2 py-1.5 input-container input";

export function EditModal(
  { style, fields, title, path }: {
    path: APIPath;
    style: "primary" | "danger";
    title: string;
    fields: Field[];
  },
) {
  const open = useSignal(false);
  const status = useSignal<"pending" | "submitting">("pending");
  const state = useSignal(
    Object.fromEntries(fields.map((field) => [field.name, field.value])),
  );
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

  useEffect(() => {
    if (!open.value && status.value !== "pending") {
      setTimeout(() => {
        status.value = "pending";
      }, 200);
    }
  }, [open]);

  const prefix = useId();

  return (
    <div class="select-none text-left">
      <button
        ref={buttonRef}
        id={`${prefix}-edit-modal`}
        class={`button-${style}`}
        type="button"
        onClick={() => open.value = !open.value}
        aria-expanded={open.value ? "true" : "false"}
      >
        edit
      </button>
      <div
        class={`fixed top-0 right-0 w-screen h-screen bg-gray-300/40 dark:bg-jsr-gray-950/70 z-80 flex justify-center items-center overflow-hidden ${
          open.value ? "opacity-100" : "opacity-0 pointer-events-none"
        } transition`}
        aria-labelledby={`${prefix}-edit-modal`}
        role="region"
        style="--tw-shadow-color: rgba(156,163,175,0.2);"
      >
        <form
          ref={ref}
          class={`space-y-3 z-90 rounded border-1.5 border-current dark:border-cyan-700 bg-white dark:bg-jsr-gray-950 shadow min-w-96 ${
            status.value === "pending" ? "w-[40vw]" : ""
          } max-w-[95vw] max-h-[95vh] px-6 py-4 ${
            open.value ? "translate-y-0" : "translate-y-5"
          } transition`}
          style="--tw-shadow-color: rgba(156,163,175,0.2);"
          onSubmit={(e) => {
            e.preventDefault();

            // deno-lint-ignore no-explicit-any
            const data: any = {};

            for (const field of fields) {
              const val = state.value[field.name];

              if (field.value !== undefined) {
                if (field.value !== val) {
                  data[field.name] = val;
                }
              } else {
                data[field.name] = val;
              }
            }

            if (Object.keys(data).length === 0) {
              open.value = false;
              return;
            }

            status.value = "submitting";

            api.patch(path, data).then((res) => {
              if (res.ok) {
                globalThis.location.reload();
              }
            });
          }}
        >
          <h2 class="text-lg font-semibold text-primary">
            {title}
          </h2>

          {status.value === "pending"
            ? (
              <>
                {fields.map((field) => {
                  let input;

                  switch (field.type) {
                    case "textarea":
                      input = (
                        <textarea
                          name={field.name}
                          required={field.required}
                          class={`${BASE_INPUT_STYLING} min-h-[4em] max-h-[20em]`}
                          value={state.value[field.name] as string}
                          onChange={(event) => {
                            state.value[field.name] = event.currentTarget.value;
                          }}
                        />
                      );
                      break;
                    case "select":
                      input = (
                        <select
                          name={field.name}
                          required={field.required}
                          class={BASE_INPUT_STYLING}
                          value={state.value[field.name] as string}
                          onChange={(event) => {
                            state.value[field.name] = event.currentTarget.value;
                          }}
                        >
                          {field.values!.map((value) => (
                            <option key={value} value={value}>{value}</option>
                          ))}
                        </select>
                      );
                      break;
                    case "boolean":
                      input = (
                        <select
                          name={field.name}
                          required={field.required}
                          class={BASE_INPUT_STYLING}
                          value={state.value[field.name] ? "true" : "false"}
                          onChange={(event) => {
                            state.value[field.name] =
                              event.currentTarget.value === "true";
                          }}
                        >
                          <option value="true">true</option>
                          <option value="false">false</option>
                        </select>
                      );
                      break;
                    case "number":
                      input = (
                        <input
                          type="number"
                          name={field.name}
                          required={field.required}
                          class={BASE_INPUT_STYLING}
                          value={state.value[field.name] as string}
                          onChange={(event) => {
                            state.value[field.name] = +event.currentTarget
                              .value;
                          }}
                        />
                      );
                      break;
                    default:
                      input = (
                        <input
                          type={field.type}
                          name={field.name}
                          required={field.required}
                          class={BASE_INPUT_STYLING}
                          value={state.value[field.name] as string}
                          onChange={(event) => {
                            state.value[field.name] = event.currentTarget.value;
                          }}
                        />
                      );
                  }

                  return (
                    <label class="block">
                      <span class="text-sm text-primary mb-1.5 block">
                        {field.label}
                        {field.required
                          ? <span class="text-sm text-red-500">*</span>
                          : null}
                      </span>
                      {input}
                    </label>
                  );
                })}

                <div class="flex justify-between">
                  <input type="submit" value="Submit" class="button-primary" />
                  <button
                    type="button"
                    class="button-danger"
                    onClick={() => {
                      open.value = false;
                      ref.current?.reset();
                    }}
                  >
                    Cancel
                  </button>
                </div>
              </>
            )
            : (
              <div class="flex flex-col gap-3 items-center justify-center py-6">
                {status.value === "submitting" && (
                  <TbLoader2 class="w-8 h-8 animate-spin" />
                )}
              </div>
            )}
        </form>
      </div>
    </div>
  );
}

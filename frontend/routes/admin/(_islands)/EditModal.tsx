// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect, useId, useRef, useState } from "preact/hooks";
import { TbLoader2 } from "tb-icons";
import { api, APIPath } from "../../../utils/api.ts";

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
  const [open, setOpen] = useState(false);
  const [status, setStatus] = useState<"pending" | "submitting">(
    "pending",
  );
  const [state, setState] = useState(
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
        setOpen(false);
      }
    }
    document.addEventListener("click", outsideClick);
    return () => document.removeEventListener("click", outsideClick);
  }, []);

  useEffect(() => {
    if (!open && status !== "pending") {
      setTimeout(() => {
        setStatus("pending");
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
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open ? "true" : "false"}
      >
        edit
      </button>
      <div
        class={`fixed top-0 right-0 w-screen h-screen bg-gray-300/40 dark:bg-jsr-gray-900/80 z-[80] flex justify-center items-center overflow-hidden ${
          open ? "opacity-100" : "opacity-0 pointer-events-none"
        } transition`}
        aria-labelledby={`${prefix}-edit-modal`}
        role="region"
        style="--tw-shadow-color: rgba(156,163,175,0.2);"
      >
        <form
          ref={ref}
          class={`space-y-3 z-[90] rounded border-1.5 border-current bg-white dark:bg-jsr-gray-800 dark:text-gray-200 shadow min-w-96 ${
            status === "pending" ? "w-[40vw]" : ""
          } max-w-[95vw] max-h-[95vh] px-6 py-4 ${
            open ? "translate-y-0" : "translate-y-5"
          } transition`}
          style="--tw-shadow-color: rgba(156,163,175,0.2);"
          onSubmit={(e) => {
            e.preventDefault();

            // deno-lint-ignore no-explicit-any
            const data: any = {};

            for (const field of fields) {
              const val = state[field.name];

              if (field.value !== undefined) {
                if (field.value !== val) {
                  data[field.name] = val;
                }
              } else {
                data[field.name] = val;
              }
            }

            if (Object.keys(data).length === 0) {
              setOpen(false);
              return;
            }

            setStatus("submitting");

            api.patch(path, data).then((res) => {
              if (res.ok) {
                globalThis.location.reload();
              }
            });
          }}
        >
          <h2 class="text-lg font-semibold">
            {title}
          </h2>

          {status === "pending"
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
                          value={state[field.name] as string}
                          onChange={(event) => {
                            setState((state) => {
                              state[field.name] = event.currentTarget.value;
                              return state;
                            });
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
                          value={state[field.name] as string}
                          onChange={(event) => {
                            setState((state) => {
                              state[field.name] = event.currentTarget.value;
                              return state;
                            });
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
                          value={state[field.name] ? "true" : "false"}
                          onChange={(event) => {
                            setState((state) => {
                              state[field.name] =
                                event.currentTarget.value === "true";
                              return state;
                            });
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
                          value={state[field.name] as string}
                          onChange={(event) => {
                            setState((state) => {
                              state[field.name] = +event.currentTarget.value;
                              return state;
                            });
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
                          value={state[field.name] as string}
                          onChange={(event) => {
                            setState((state) => {
                              state[field.name] = event.currentTarget.value;
                              return state;
                            });
                          }}
                        />
                      );
                  }

                  return (
                    <label class="block">
                      <span class="text-sm">
                        {field.label}
                        {field.required
                          ? <span class="text-sm text-red-500">*</span>
                          : null}
                      </span>
                      <br />
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
                      setOpen(false);
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
                {status === "submitting" && (
                  <TbLoader2 class="w-8 h-8 animate-spin" />
                )}
              </div>
            )}
        </form>
      </div>
    </div>
  );
}

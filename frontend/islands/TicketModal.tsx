// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect, useId, useRef } from "preact/hooks";
import { ApiTicket, NewTicket, TicketKind, User } from "../utils/api_types.ts";
import type { ComponentChildren } from "preact";
import { TbLoader2 } from "tb-icons";
import { api, path } from "../utils/api.ts";
import { useSignal } from "@preact/signals";

interface Field {
  name: string;
  label: string;
  type: string;
  values?: string[];
  required: boolean;
}

const BASE_INPUT_STYLING = "w-full block px-2 py-1.5 input-container input";

export function TicketModal(
  { user, kind, style, fields, children, extraMeta, title, description }: {
    children: ComponentChildren;
    kind: TicketKind;
    style: "primary" | "danger";
    user: User | null;
    title: string;
    description: ComponentChildren;
    fields: Field[];
    extraMeta?: Record<string, string | undefined>;
  },
) {
  const open = useSignal(false);
  const status = useSignal<"pending" | "submitting" | "submitted">("pending");
  const ticket = useSignal<ApiTicket | null>(null);
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
        id={`${prefix}-ticket-modal`}
        class={`button-${style}`}
        type="button"
        onClick={() => open.value = !open.value}
        aria-expanded={open.value ? "true" : "false"}
        disabled={!user}
        title={user ? "" : "Please log-in to use this button"}
      >
        {children}
      </button>
      <div
        class={`fixed top-0 right-0 w-screen h-screen bg-gray-300/40 dark:bg-jsr-gray-950/70 z-[80] flex justify-center items-center overflow-hidden ${
          open.value ? "opacity-100" : "opacity-0 pointer-events-none"
        } transition`}
        aria-labelledby={`${prefix}-ticket-modal`}
        role="region"
        style="--tw-shadow-color: rgba(156,163,175,0.2);"
      >
        <form
          ref={ref}
          class={`space-y-3 z-[90] rounded border-1.5 border-current dark:border-cyan-700 bg-white dark:bg-jsr-gray-950 shadow min-w-96 ${
            status.value === "pending" ? "w-[40vw]" : ""
          } max-w-[95vw] max-h-[95vh] px-6 py-4 ${
            open.value ? "translate-y-0" : "translate-y-5"
          } transition`}
          style="--tw-shadow-color: rgba(156,163,175,0.2);"
          onSubmit={(e) => {
            e.preventDefault();
            const formdata = new FormData(e.currentTarget);

            const meta = Object.fromEntries(
              formdata.entries().filter(([_, v]) => typeof v === "string"),
            ) as Record<string, string>;

            const message = meta.message as string;
            delete meta.message;

            if (extraMeta) {
              Object.assign(meta, extraMeta);
            }

            const data: NewTicket = {
              kind,
              message,
              meta,
            };

            status.value = "submitting";

            api.post<ApiTicket>(path`/tickets`, data).then((res) => {
              if (res.ok) {
                status.value = "submitted";
                ticket.value = res.data;
              }
            });
          }}
        >
          <h2 class="text-lg font-semibold text-primary">
            New Ticket: {title}
          </h2>

          {status.value === "pending"
            ? (
              <>
                <div class="text-sm text-secondary">
                  {description}
                </div>

                {fields.map((field) => {
                  let input;

                  switch (field.type) {
                    case "textarea":
                      input = (
                        <textarea
                          name={field.name}
                          required={field.required}
                          class={`${BASE_INPUT_STYLING} min-h-[4em] max-h-[20em]`}
                        />
                      );
                      break;
                    case "select":
                      input = (
                        <select
                          name={field.name}
                          required={field.required}
                          class={BASE_INPUT_STYLING}
                        >
                          {field.values!.map((value) => (
                            <option key={value} value={value}>{value}</option>
                          ))}
                        </select>
                      );
                      break;
                    default:
                      input = (
                        <input
                          type={field.type}
                          name={field.name}
                          required={field.required}
                          class={BASE_INPUT_STYLING}
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
                {status.value === "submitting"
                  ? <TbLoader2 class="w-8 h-8 animate-spin" />
                  : (
                    <>
                      <div>
                        The ticket was submitted. You can view it{" "}
                        <a href={`/ticket/${ticket.value!.id}`} class="link">
                          here
                        </a>
                      </div>
                      <button
                        type="button"
                        class="button-danger"
                        onClick={() => {
                          open.value = false;
                          ref.current?.reset();
                        }}
                      >
                        Close
                      </button>
                    </>
                  )}
              </div>
            )}
        </form>
      </div>
    </div>
  );
}

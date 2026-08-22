// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { JSX } from "preact";
import TbMail from "tb-icons/TbMail";
import TbRobot from "tb-icons/TbRobot";
import type { ApiTicketActor } from "../utils/api_types.ts";

export function ticketActorName(actor: ApiTicketActor): string {
  switch (actor.kind) {
    case "user":
      return actor.user.name;
    case "email":
      return actor.name ?? actor.email;
    case "system":
      return "JSR";
  }
}

/// Renders whoever wrote a message or opened a ticket: a JSR account with its
/// avatar, an email address for a ticket nobody has claimed, or JSR itself for
/// the automatic acknowledgement.
export function TicketActor(
  { actor }: { actor: ApiTicketActor },
): JSX.Element {
  switch (actor.kind) {
    case "user":
      return (
        <a class="contents" href={`/user/${actor.user.id}`}>
          <img
            src={actor.user.avatarUrl}
            class="w-7 aspect-square rounded-full ring-2 ring-jsr-cyan-700 select-none"
            alt={actor.user.name}
          />
          <span class="font-semibold">{actor.user.name}</span>
        </a>
      );
    case "email":
      return (
        <>
          <div class="w-7 aspect-square rounded-full bg-jsr-gray-200 dark:bg-jsr-gray-700 flex items-center justify-center">
            <TbMail class="text-jsr-gray-700 dark:text-jsr-gray-200" />
          </div>
          <span class="font-semibold" title={actor.email}>
            {actor.name ?? actor.email}
          </span>
          {!actor.emailVerified && (
            // The sending domain failed SPF or DKIM, so the address proves
            // nothing about who actually sent this.
            <span
              class="rounded-full text-sm px-2 inline-block bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-100"
              title="This email failed SPF or DKIM checks, so the sender address is unverified."
            >
              unverified sender
            </span>
          )}
        </>
      );
    case "system":
      return (
        <>
          <div class="w-7 aspect-square rounded-full bg-jsr-gray-200 dark:bg-jsr-gray-700 flex items-center justify-center">
            <TbRobot class="text-jsr-gray-700 dark:text-jsr-gray-200" />
          </div>
          <span class="font-semibold">JSR</span>
        </>
      );
  }
}

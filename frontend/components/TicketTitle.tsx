// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { JSX } from "preact";
import type { ApiTicketActor, TicketKind } from "../utils/api_types.ts";
import { ticketActorName } from "./TicketActor.tsx";

interface TicketTitleProps {
  kind: TicketKind;
  meta: Record<string, string>;
  reporter: ApiTicketActor;
  /// The email subject, for a ticket that arrived by email. Such tickets have no
  /// structured `kind`/`meta` to build a title from, so this is used instead.
  subject?: string | null;
}

export function TicketTitle(props: TicketTitleProps): JSX.Element {
  if (props.subject) {
    return <>{props.subject}</>;
  }

  let title: string;
  switch (props.kind) {
    case "other":
      title = "Other";
      break;
    case "user_scope_quota_increase":
      title = `Request scope quota increase for '${
        ticketActorName(props.reporter)
      }'`;
      break;
    case "scope_quota_increase":
      title = `Request '${
        props.meta["quota kind"]
      }' quota increase for '@${props.meta.scope}'`;
      break;
    case "scope_claim":
      title = `Request for reserved scope '@${props.meta.scope}'`;
      break;
    case "staff_outreach":
      // Always opened with a subject; this is only reached if it was blank.
      title = "Message from JSR staff";
      break;
    case "package_report":
      title = `Report package '${props.meta.scope}/${props.meta.name}${
        props.meta.version ? `@${props.meta.version}` : ""
      }'`;
      break;
    default:
      title = "Unknown ticket kind";
  }
  return <>{title}</>;
}

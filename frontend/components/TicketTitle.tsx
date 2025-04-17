// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { JSX } from "preact";
import type { TicketKind, User } from "../utils/api_types.ts";

interface TicketTitleProps {
  kind: TicketKind;
  meta: Record<string, string>;
  user: User;
}

export function TicketTitle(props: TicketTitleProps): JSX.Element {
  let title: string;
  switch (props.kind) {
    case "other":
      title = "Other";
      break;
    case "user_scope_quota_increase":
      title = `Request scope quota increase for '${props.user.name}'`;
      break;
    case "scope_quota_increase":
      title = `Request '${
        props.meta["quota kind"]
      }' quota increase for '@${props.meta.scope}'`;
      break;
    case "scope_claim":
      title = `Request for reserved scope '@${props.meta.scope}'`;
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

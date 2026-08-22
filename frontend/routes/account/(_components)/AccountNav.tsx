// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Nav, NavItem } from "../../../components/Nav.tsx";
import { FullUser } from "../../../utils/api_types.ts";

export type AccountNavTab =
  | "Profile"
  | "Invites"
  | "Tokens"
  | "Settings"
  | "Tickets";

export interface AccountNavProps {
  user: FullUser;
  active: AccountNavTab;
}

export function AccountNav(props: AccountNavProps) {
  return (
    <Nav>
      <NavItem
        href={`/user/${props.user.id}`}
        active={props.active === "Profile"}
      >
        Profile
      </NavItem>
      <NavItem
        href="/account/invites"
        active={props.active === "Invites"}
        chip={props.user.inviteCount}
        notification
      >
        Invites
      </NavItem>
      <NavItem
        href={`/account/tokens`}
        active={props.active === "Tokens"}
      >
        Tokens
      </NavItem>
      <NavItem
        href={`/account/settings`}
        active={props.active === "Settings"}
      >
        Settings
      </NavItem>
      <NavItem
        href={`/account/tickets`}
        active={props.active === "Tickets"}
        chip={props.user.newerTicketMessagesCount}
        notification
      >
        Support Tickets
      </NavItem>
    </Nav>
  );
}

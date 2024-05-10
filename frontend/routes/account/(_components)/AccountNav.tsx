// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Nav, NavItem } from "../../../components/Nav.tsx";
import { FullUser } from "../../../utils/api_types.ts";

export type AccountNavTab = "Profile" | "Invites" | "Tokens" | "Settings";

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
        href={"/account/invites"}
        active={props.active === "Invites"}
      >
        <span class="flex items-center">
          Invites
          <span
            class={`chip ml-2 tabular-nums ${
              props.user.inviteCount > 0
                ? "bg-orange-600 text-white"
                : "bg-gray-200"
            }`}
          >
            {props.user.inviteCount}
          </span>
        </span>
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
    </Nav>
  );
}

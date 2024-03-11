// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Nav, NavItem } from "../../../components/Nav.tsx";
import { ScopeIAM } from "../../../utils/iam.ts";

export type ScopeNavTab = "Packages" | "Members" | "Settings";

export interface ScopeNavProps {
  scope: string;
  active: ScopeNavTab;
  iam: ScopeIAM;
}

export function ScopeNav(props: ScopeNavProps) {
  const baseUrl = `/@${props.scope}`;
  return (
    <Nav>
      <NavItem href={baseUrl} active={props.active === "Packages"}>
        Packages
      </NavItem>
      <NavItem
        href={`${baseUrl}/~/members`}
        active={props.active === "Members"}
      >
        Members
      </NavItem>
      {props.iam.canAdmin && (
        <NavItem
          href={`${baseUrl}/~/settings`}
          active={props.active === "Settings"}
        >
          Settings
        </NavItem>
      )}
    </Nav>
  );
}

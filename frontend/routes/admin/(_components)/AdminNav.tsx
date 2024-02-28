// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Nav, NavItem } from "../../../components/Nav.tsx";

export interface Params {
  scope: string;
  package: string;
  version?: string;
}

type Tab = "scopes" | "users" | "waitlist" | "publishingTasks";

export function AdminNav({ currentTab }: {
  currentTab: Tab;
}) {
  return (
    <>
      <h1 class="font-bold text-2xl">Admin</h1>
      <Nav>
        <NavItem href="/admin/scopes" active={currentTab === "scopes"}>
          Scopes
        </NavItem>
        <NavItem href="/admin/users" active={currentTab === "users"}>
          Users
        </NavItem>
        <NavItem href="/admin/waitlist" active={currentTab === "waitlist"}>
          Waitlist
        </NavItem>
        <NavItem
          href="/admin/publishingTasks"
          active={currentTab === "publishingTasks"}
        >
          Publishing Tasks
        </NavItem>
      </Nav>
    </>
  );
}

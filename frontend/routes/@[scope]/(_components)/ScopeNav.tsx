// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Nav, NavItem } from "../../../components/Nav.tsx";
import { ScopeIAM } from "../../../utils/iam.ts";
import { ScopeSymbolSearch } from "../(_islands)/ScopeSymbolSearch.tsx";

export type ScopeNavTab = "Packages" | "Members" | "Settings";

export interface ScopeNavProps {
  scope: string;
  active: ScopeNavTab;
  iam: ScopeIAM;
}

const oramaApiKey = Deno.env.get("ORAMA_SYMBOLS_PUBLIC_API_KEY");
const oramaIndexId = Deno.env.get("ORAMA_SYMBOLS_PUBLIC_INDEX_ID");

export function ScopeNav(props: ScopeNavProps) {
  const baseUrl = `/@${props.scope}`;
  return (
    <Nav
      end={
        <div>
          <ScopeSymbolSearch
            scope={props.scope}
            indexId={oramaIndexId}
            apiKey={oramaApiKey}
          />
        </div>
      }
    >
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

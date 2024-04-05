// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Nav, NavItem } from "../../../components/Nav.tsx";
import { ScopeIAM } from "../../../utils/iam.ts";

export interface Params {
  scope: string;
  package: string;
  version?: string;
}

type Tab =
  | "Index"
  | "Symbols"
  | "Files"
  | "Versions"
  | "Dependencies"
  | "Dependents"
  | "Score"
  | "Publish"
  | "Settings";

export function PackageNav(
  { currentTab, params, iam, versionCount, latestVersion }: {
    currentTab: Tab;
    params: Params;
    versionCount: number;
    iam: ScopeIAM;
    latestVersion: string | null;
  },
) {
  const base = `/@${params.scope}/${params.package}`;
  const versionedBase = `${base}${params.version ? `@${params.version}` : ""}`;

  return (
    <Nav noTopMargin>
      {((iam.canWrite && versionCount > 0) || !iam.canWrite) && (
        <NavItem href={versionedBase} active={currentTab === "Index"}>
          Overview
        </NavItem>
      )}
      {(latestVersion || params.version) && (
        <NavItem
          href={`${versionedBase}/doc`}
          active={currentTab === "Symbols"}
        >
          Symbols
        </NavItem>
      )}
      {(latestVersion || params.version) && (
        <NavItem
          href={`${base}/${params.version || latestVersion}`}
          active={currentTab === "Files"}
        >
          Files
        </NavItem>
      )}
      <NavItem href={`${base}/versions`} active={currentTab === "Versions"}>
        <span class="flex items-center">
          Versions
          <span class="chip tabular-nums bg-jsr-cyan-200 ml-2 leading-[0] w-[1.5em] aspect-square flex items-center justify-center">
            {versionCount}
          </span>
        </span>
      </NavItem>
      {(latestVersion || params.version) && (
        <NavItem
          href={`${versionedBase}/dependencies`}
          active={currentTab === "Dependencies"}
        >
          Dependencies
        </NavItem>
      )}
      {versionCount > 0 && (
        <NavItem
          href={`${base}/dependents`}
          active={currentTab === "Dependents"}
        >
          Dependents
        </NavItem>
      )}
      {versionCount > 0 && (
        <NavItem
          href={`${base}/score`}
          active={currentTab === "Score"}
        >
          Score
        </NavItem>
      )}
      {iam.canWrite &&
        (
          <NavItem
            href={`${base}/publish`}
            active={currentTab === "Publish"}
          >
            Publish
          </NavItem>
        )}
      {iam.canAdmin &&
        (
          <NavItem
            href={`${base}/settings`}
            active={currentTab === "Settings"}
          >
            Settings
          </NavItem>
        )}
    </Nav>
  );
}

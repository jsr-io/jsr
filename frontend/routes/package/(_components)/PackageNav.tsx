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
  | "Docs"
  | "Diff"
  | "Files"
  | "Versions"
  | "Dependencies"
  | "Dependents"
  | "Score"
  | "Publish"
  | "Settings";

interface PackageNavProps {
  currentTab: Tab;
  params: Params;
  versionCount: number;
  dependencyCount: number;
  dependentCount: number;
  /** Symbols documented by the version being viewed. Null for versions
   * published before the count was recorded, which show no chip rather than
   * claiming the package has no symbols. */
  symbolCount?: number | null;
  iam: ScopeIAM;
  latestVersion: string | null;
}

export function PackageNav({
  currentTab,
  params,
  iam,
  versionCount,
  dependencyCount,
  dependentCount,
  symbolCount,
  latestVersion,
}: PackageNavProps) {
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
          active={currentTab === "Docs"}
          chip={symbolCount ?? undefined}
        >
          Docs
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
      <NavItem
        href={`${base}/versions`}
        active={currentTab === "Versions"}
        chip={versionCount}
      >
        Versions
      </NavItem>
      {(latestVersion || params.version) && (
        <NavItem
          href={`${versionedBase}/dependencies`}
          active={currentTab === "Dependencies"}
          chip={dependencyCount}
        >
          Dependencies
        </NavItem>
      )}
      {versionCount > 0 && (
        <NavItem
          href={`${base}/dependents`}
          active={currentTab === "Dependents"}
          chip={dependentCount}
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

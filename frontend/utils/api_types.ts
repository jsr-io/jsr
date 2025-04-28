// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export interface User {
  id: string;
  name: string;
  githubId: number | null;
  avatarUrl: string;
  updatedAt: string;
  createdAt: string;
}

export interface FullUser extends User {
  email: string | null;
  isStaff: boolean;
  isBlocked: boolean;
  scopeUsage: number;
  scopeLimit: number;
  inviteCount: number;
  newerTicketMessagesCount: number;
}

export interface Scope {
  scope: string;
  updatedAt: string;
  createdAt: string;
}

export interface FullScope extends Scope {
  creator: User;
  quotas: ScopeQuota;
  ghActionsVerifyActor: boolean;
  requirePublishingFromCI: boolean;
}

export interface ScopeQuota {
  packageUsage: number;
  packageLimit: number;
  newPackagePerWeekUsage: number;
  newPackagePerWeekLimit: number;
  publishAttemptsPerWeekUsage: number;
  publishAttemptsPerWeekLimit: number;
}

export interface ScopeMember {
  scope: string;
  user: User;
  isAdmin: boolean;
  updatedAt: string;
  createdAt: string;
}

export interface ScopeInvite {
  scope: string;
  targetUser: User;
  requestingUser: User;
  updatedAt: string;
  createdAt: string;
}

export type PublishingTaskStatus =
  | "pending"
  | "processing"
  | "processed"
  | "success"
  | "failure";

export interface PublishingTask {
  id: string;
  status: PublishingTaskStatus;
  error: { code: string; message: string } | null;
  user: User | null;
  packageScope: string;
  packageName: string;
  packageVersion: string;
  createdAt: string;
  updatedAt: string;
}

export interface GithubRepository {
  id: number;
  owner: string;
  name: string;
  updatedAt: string;
  createdAt: string;
}

export interface RuntimeCompat {
  browser?: boolean;
  deno?: boolean;
  node?: boolean;
  workerd?: boolean;
  bun?: boolean;
}

export interface PackageScore {
  hasReadme: boolean;
  hasReadmeExamples: boolean;
  allEntrypointsDocs: boolean;
  percentageDocumentedSymbols: number;
  allFastCheck: boolean;
  hasProvenance: boolean;

  // package specific
  hasDescription: boolean;
  atLeastOneRuntimeCompatible: boolean;
  multipleRuntimesCompatible: boolean;

  total: number;
}

export interface Package {
  scope: string;
  name: string;
  description: string;
  githubRepository: GithubRepository | null;
  runtimeCompat: RuntimeCompat;
  updatedAt: string;
  createdAt: string;
  versionCount: number;
  dependencyCount: number;
  dependentCount: number;
  score: number | null;
  latestVersion: string | null;
  whenFeatured: string | null;
  isArchived: boolean;
}

export interface PackageVersion {
  scope: string;
  package: string;
  version: string;
  yanked: boolean;
  usesNpm: boolean;
  newerVersionsCount: number;
  rekorLogId: string | null;
  readmePath: string;
  updatedAt: string;
  createdAt: string;
}

export interface PackageVersionWithUser extends PackageVersion {
  user?: User;
}

export interface PackageVersionDocsContent {
  kind: "content";
  version: PackageVersionWithUser;
  css: string;
  comrakCss: string;
  script: string;
  breadcrumbs: string | null;
  toc: string | null;
  main: string;
}

export interface PackageVersionDocsRedirect {
  kind: "redirect";
  symbol: string;
}

export type PackageVersionDocs =
  | PackageVersionDocsContent
  | PackageVersionDocsRedirect;

export interface SourceDirEntry {
  name: string;
  size: number;
  kind: "dir" | "file";
}

export interface SourceDir {
  kind: "dir";
  entries: SourceDirEntry[];
}

export interface SourceFile {
  kind: "file";
  size: number;
  view: string | null;
}

export interface PackageVersionSource {
  version: PackageVersionWithUser;
  css: string;
  comrakCss: string;
  script: string;
  source: SourceDir | SourceFile;
}

export interface Authorization {
  code: string;
  permissions: Permission[] | null;
  expiresAt: string;
}

export type PermissionPackagePublishScope = {
  permission: "package/publish";
  scope: string;
};

export type PermissionPackagePublishPackage = {
  permission: "package/publish";
  scope: string;
  package: string;
};

export type PermissionPackagePublishVersion = {
  permission: "package/publish";
  scope: string;
  package: string;
  version: string;
  tarballHash: string;
};

export type Permission =
  | PermissionPackagePublishScope
  | PermissionPackagePublishPackage
  | PermissionPackagePublishVersion;

export interface Dependency {
  kind: "jsr" | "npm";
  name: string;
  constraint: string;
  path: string;
}

export interface PackageVersionReference {
  scope: string;
  package: string;
  version: string;
}

export interface Stats {
  newest: Package[];
  updated: PackageVersionWithUser[];
  featured: Package[];
}

export interface List<T> {
  items: T[];
  total: number;
}

export interface Dependent {
  scope: string;
  package: string;
  versions: string[];
  totalVersions: number;
}

export interface Token {
  id: string;
  description: string | null;
  type: "web" | "device" | "personal";
  expiresAt: string | null;
  permissions: Permission[] | null;
  updatedAt: string;
  createdAt: string;
}

export interface CreatedToken {
  token: Token;
  secret: string;
}

export interface DependencyGraphJsrEntrypoint {
  type: "entrypoint" | "path";
  value: string;
}

export interface DependencyGraphKindJsr {
  type: "jsr";
  scope: string;
  package: string;
  version: string;
  entrypoint: DependencyGraphJsrEntrypoint;
}

export interface DependencyGraphKindNpm {
  type: "npm";
  package: string;
  version: string;
}
export interface DependencyGraphKindRoot {
  type: "root";
  path: string;
}

export interface DependencyGraphKindError {
  type: "error";
  error: string;
}

export type DependencyGraphKind =
  | DependencyGraphKindJsr
  | DependencyGraphKindNpm
  | DependencyGraphKindRoot
  | DependencyGraphKindError;

export interface DependencyGraphItem {
  id: number;
  dependency: DependencyGraphKind;
  children: number[];
  size: number | undefined;
  mediaType: string | undefined;
}

export type TicketKind =
  | "user_scope_quota_increase"
  | "scope_quota_increase"
  | "scope_claim"
  | "package_report"
  | "other";

export interface NewTicket {
  kind: TicketKind;
  meta?: Record<string, string>;
  message: string;
}

export interface NewTicketMessage {
  message: string;
}

export interface Ticket {
  id: string;
  kind: TicketKind;
  creator: User;
  meta: Record<string, string>;
  closed: boolean;
  messages: TicketMessage[];
  updatedAt: string;
  createdAt: string;
}

export interface TicketMessage {
  author: User;
  message: string;
  updatedAt: string;
  createdAt: string;
}

export interface AdminUpdateTicketRequest {
  closed?: boolean;
}

export interface AuditLog {
  actor: User;
  isSudo: boolean;
  action: string;
  meta: Record<string, unknown>;
  createdAt: string;
}

export interface PackageDownloads {
  total: DownloadDataPoint[];
  recentVersions: PackageDownloadsRecentVersion[];
}

export interface DownloadDataPoint {
  timeBucket: string;
  kind: "jsr_meta" | "npm_tarball";
  count: number;
}

export interface PackageDownloadsRecentVersion {
  version: string;
  downloads: DownloadDataPoint[];
}

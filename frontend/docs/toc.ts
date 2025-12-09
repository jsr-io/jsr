// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export interface TOCEntry {
  title: string;
  id: string;
  group: typeof groupsNames[number];
}

export const groupsNames = [
  "Guide",
  "Reference",
  "Governance",
  "Use JSR with",
] as const;

export default [
  {
    title: "Introduction",
    id: "introduction",
    group: "Guide",
  },
  {
    title: "Using packages",
    id: "using-packages",
    group: "Guide",
  },
  {
    title: "Publishing packages",
    id: "publishing-packages",
    group: "Guide",
  },
  {
    title: "Writing documentation",
    id: "writing-docs",
    group: "Guide",
  },
  {
    title: "Troubleshooting",
    id: "troubleshooting",
    group: "Guide",
  },
  {
    title: "Migrate from /x",
    id: "migrate-x-to-jsr",
    group: "Guide",
  },
  {
    title: "Other registries",
    id: "other-registries",
    group: "Guide",
  },
  {
    title: "Why JSR?",
    id: "why",
    group: "Guide",
  },
  {
    "title": "FAQ",
    "id": "faq",
    group: "Guide",
  },
  {
    title: "Scopes",
    id: "scopes",
    group: "Reference",
  },
  {
    title: "Packages",
    id: "packages",
    group: "Reference",
  },
  {
    title: "`jsr.json` file",
    id: "package-configuration",
    group: "Reference",
  },
  {
    title: "`jsr:` imports",
    id: "native-imports",
    group: "Reference",
  },
  {
    title: "npm compatibility",
    id: "npm-compatibility",
    group: "Reference",
  },
  {
    title: 'About "slow types"',
    id: "about-slow-types",
    group: "Reference",
  },
  {
    title: "Scoring",
    id: "scoring",
    group: "Reference",
  },
  {
    title: "Private registries",
    id: "private-registries",
    group: "Reference",
  },
  {
    title: "Provenance and trust",
    id: "trust",
    group: "Reference",
  },
  {
    title: "Badges",
    id: "badges",
    group: "Reference",
  },
  {
    title: "Immutability",
    id: "immutability",
    group: "Reference",
  },
  {
    title: "Quotas and limits",
    id: "quotas-and-limits",
    group: "Reference",
  },
  {
    title: "Usage policy",
    id: "usage-policy",
    group: "Reference",
  },
  {
    title: "API",
    id: "api",
    group: "Reference",
  },
  {
    title: "Overview",
    id: "governance",
    group: "Governance",
  },
  {
    title: "Charter Document",
    id: "governance/charter",
    group: "Governance",
  },
  {
    title: "Deno",
    id: "with/deno",
    group: "Use JSR with",
  },
  {
    title: "Node.js",
    id: "with/node",
    group: "Use JSR with",
  },
  {
    title: "Cloudflare Workers",
    id: "with/cloudflare-workers",
    group: "Use JSR with",
  },
  {
    title: "Vite",
    id: "with/vite",
    group: "Use JSR with",
  },
  {
    title: "Next.js",
    id: "with/nextjs",
    group: "Use JSR with",
  },
  {
    title: "...everything else",
    id: "with/other",
    group: "Use JSR with",
  },
] satisfies TOCEntry[];

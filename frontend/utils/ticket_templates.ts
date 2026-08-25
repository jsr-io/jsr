// Copyright 2024 the JSR authors. All rights reserved. MIT license.

/// Canned replies for the situations that come up often enough to have a house
/// answer, taken from the support runbook.
///
/// Kept here rather than in the database: they are house policy, they change
/// about as often as the policy pages they link to, and having them in the repo
/// means a wording change gets reviewed like any other user-facing copy. If
/// support ends up wanting to edit them without a deploy, they can move.
export interface TicketTemplate {
  id: string;
  /// Groups the templates in the picker.
  category: string;
  label: string;
  body: string;
}

/// Anything a person has to replace before sending is written in square
/// brackets, matching the runbook. `PLACEHOLDER_PATTERN` finds them again so the
/// reply box can warn about ones left behind.
export const PLACEHOLDER_PATTERN = /\[[a-z0-9_ ]+\]/gi;

export const TICKET_TEMPLATES: TicketTemplate[] = [
  {
    id: "scope-quota-approved",
    category: "Scope quota",
    label: "Approved",
    body:
      "I've increased your scope quota to [new_limit]. Please remember our policy on scope name squatting: https://jsr.io/docs/usage-policy#scope-name-squatting",
  },
  {
    id: "scope-quota-still-has-quota",
    category: "Scope quota",
    label: "Denied — quota not used up",
    body:
      "We can't raise your scope limit yet since you haven't used your current quota. Please use your existing slots before requesting more. Review our policy on scope names: https://jsr.io/docs/usage-policy#scope-name-squatting. Let us know if you have questions.",
  },
  {
    id: "scope-quota-unused-scopes",
    category: "Scope quota",
    label: "Denied — scopes unused",
    body:
      "We can't raise your scope limit yet since you're not using your current scopes. Please publish real packages to your existing scopes before requesting more. Review our policy on scope names: https://jsr.io/docs/usage-policy#scope-name-squatting. Let us know if you have questions.",
  },
  {
    id: "scope-quota-generic-names",
    category: "Scope quota",
    label: "Denied — generic names",
    body:
      "We can't raise your scope limit as you've registered multiple generic names:\n\n- [scope1]\n- [scope2]\n\nOur guidelines require scope names to be relevant to your packages without being overly generic: https://jsr.io/docs/usage-policy#name-guidelines. Let us know if you have questions.",
  },
  {
    id: "package-quota-approved",
    category: "Package quota",
    label: "Approved",
    body: "I've increased your [quota_name] to [new_limit].",
  },
  {
    id: "package-size-approved",
    category: "Package size",
    label: "Approved",
    body:
      "I've increased the maximum size for your package to [new_size]. Note that we cannot exceed the 32MB hard limit.",
  },
  {
    id: "package-size-denied",
    category: "Package size",
    label: "Denied — split the package",
    body:
      "We aren't able to raise the size limit for this package. Packages above the limit need to be split into smaller packages instead. [reason]",
  },
  {
    id: "reserved-scope-approved",
    category: "Reserved scopes",
    label: "Approved",
    body:
      "I've assigned the @[scope_name] scope to you. You can access it at: https://jsr.io/@[scope_name]. To start publishing on JSR, check out: https://jsr.io/docs/publishing-packages",
  },
  {
    id: "reserved-scope-denied",
    category: "Reserved scopes",
    label: "Denied — not eligible",
    body:
      "We're only able to release reserved scope names to people representing the project or company the name refers to, and we weren't able to verify that here. [reason]",
  },
  {
    id: "deletion-yank-instead",
    category: "Deletions",
    label: "Yank instead of delete",
    body:
      "We generally maintain immutability for packages: https://jsr.io/docs/immutability. For most cases, yanking is the recommended approach: https://jsr.io/docs/packages#yanking-versions",
  },
];

/// The placeholders still present in a draft, deduplicated and in the order they
/// appear.
export function remainingPlaceholders(message: string): string[] {
  const found = message.match(PLACEHOLDER_PATTERN) ?? [];
  return [...new Set(found)];
}

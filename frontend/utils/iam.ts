// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { State } from "../util.ts";
import { FullUser } from "./api_types.ts";
import { ScopeMember } from "./api_types.ts";

export interface ScopeIAM {
  isStaff: boolean;
  canAdmin: boolean;
  canWrite: boolean;
  hasSudo: boolean;
}

export function scopeIAM(
  state: State,
  scopeMember: ScopeMember | null,
  user?: FullUser | null,
): ScopeIAM {
  const isStaff = !!(user ?? state.user)?.isStaff;
  const hasSudo = isStaff && state.sudo;
  const canWrite = scopeMember !== null || hasSudo;
  const canAdmin = !!scopeMember?.isAdmin || hasSudo;
  return {
    isStaff,
    canAdmin,
    canWrite,
    hasSudo,
  };
}

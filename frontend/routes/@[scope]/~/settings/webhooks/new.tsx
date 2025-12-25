// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError } from "fresh";
import { define } from "../../../../../util.ts";
import { ScopeHeader } from "../../../(_components)/ScopeHeader.tsx";
import { ScopeNav } from "../../../(_components)/ScopeNav.tsx";
import { WebhookEdit } from "../../../../../islands/WebhookEdit.tsx";
import {
  FullScope,
} from "../../../../../utils/api_types.ts";
import { scopeDataWithMember } from "../../../../../utils/data.ts";
import { path } from "../../../../../utils/api.ts";
import { scopeIAM } from "../../../../../utils/iam.ts";

export default define.page<typeof handler>(function ScopeSettingsPage(
  { data },
) {
  return (
    <div class="mb-20">
      <ScopeHeader scope={data.scope} />
      <ScopeNav active="Settings" iam={data.iam} scope={data.scope.scope} />
      <WebhookEdit webhook={null} />
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const [user, data] = await Promise.all([
      ctx.state.userPromise,
      scopeDataWithMember(ctx.state, ctx.params.scope),
    ]);
    if (user instanceof Response) return user;
    if (data === null) throw new HttpError(404, "The scope was not found.");

    const iam = scopeIAM(ctx.state, data?.scopeMember, user);
    if (!iam.canAdmin) throw new HttpError(404, "The scope was not found.");

    ctx.state.meta = { title: `Webhook Settings - @${data.scope.scope} - JSR` };
    return {
      data: {
        scope: data.scope as FullScope,
        iam,
      },
    };
  },
  async POST(ctx) {
    const req = ctx.req;
    const scope = ctx.params.scope;
    const form = await req.formData();
    const action = String(form.get("action"));
    let enableGhActionsVerifyActor = false;
    switch (action) {
      case "enableGhActionsVerifyActor":
        enableGhActionsVerifyActor = true;
        // fallthrough
      case "disableGhActionsVerifyActor": {
        const res = await ctx.state.api.patch(
          path`/scopes/${scope}`,
          { ghActionsVerifyActor: enableGhActionsVerifyActor },
        );
        if (!res.ok) {
          if (res.code === "scopeNotFound") {
            throw new HttpError(404, "The scope was not found.");
          }
          throw res; // graceful handle errors
        }
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}/~/settings` },
        });
      }
      case "requirePublishingFromCI": {
        const value = form.get("value") === "true";
        const res = await ctx.state.api.patch(
          path`/scopes/${scope}`,
          { requirePublishingFromCI: value },
        );
        if (!res.ok) {
          if (res.code === "scopeNotFound") {
            throw new HttpError(404, "The scope was not found.");
          }
          throw res; // graceful handle errors
        }
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}/~/settings` },
        });
      }
      case "deleteScope": {
        const res = await ctx.state.api.delete(path`/scopes/${scope}`);
        if (!res.ok) {
          if (res.code === "scopeNotFound") {
            throw new HttpError(404, "The scope was not found.");
          }
          throw res; // graceful handle errors
        }
        return new Response(null, {
          status: 303,
          headers: { Location: `/` },
        });
      }
      default:
        throw new Error("Invalid action " + action);
    }
  },
});

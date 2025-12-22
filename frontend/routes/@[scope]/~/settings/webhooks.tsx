// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import { ComponentChildren } from "preact";
import { TbCheck, TbTrash } from "tb-icons";
import { define } from "../../../../util.ts";
import { ScopeHeader } from "../../(_components)/ScopeHeader.tsx";
import { ScopeNav } from "../../(_components)/ScopeNav.tsx";
import { ScopeDescriptionForm } from "../../(_islands)/ScopeDescriptionForm.tsx";
import {
  FullScope,
  WebhookEndpoint,
} from "../../../../utils/api_types.ts";
import { scopeDataWithMember } from "../../../../utils/data.ts";
import { path } from "../../../../utils/api.ts";
import { scopeIAM } from "../../../../utils/iam.ts";

const events = [
  {
    id: "package_version_published",
    name: "Package version published",
    description: "A new version of a package is published.",
  },
  {
    id: "package_version_yanked",
    name: "Package version yanked",
    description: "A version of a package is yanked.",
  },
  {
    id: "package_version_deleted",
    name: "Package version deleted",
    description: "A version of a package is deleted.",
  },
  {
    id: "scope_package_created",
    name: "Scope package created",
    description: "A new package is created in the scope.",
  },
  {
    id: "scope_package_archived",
    name: "Scope package archived",
    description: "A package in the scope is archived.",
  },
  {
    id: "scope_member_added",
    name: "Scope member added",
    description: "A new member is added to the scope.",
  },
  {
    id: "scope_member_left",
    name: "Scope member left",
    description: "A member leaves the scope.",
  },
]

export default define.page<typeof handler>(function ScopeSettingsPage(
  { data, state },
) {
  return (
    <div class="mb-20">
      <ScopeHeader scope={data.scope} />
      <ScopeNav active="Settings" iam={data.iam} scope={data.scope.scope} />
      <div class="mt-8">
        <div>
          <h2 class="text-lg sm:text-xl font-semibold">Description</h2>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-5 mt-3">

          </div>
        </div>
        <div>
          <h2 class="text-lg sm:text-xl font-semibold">URL</h2>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-5 mt-3">

          </div>
        </div>
        <div>
          <h2 class="text-lg sm:text-xl font-semibold">Payload format</h2>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-5 mt-3">

          </div>
        </div>
        <div>
          <h2 class="text-lg sm:text-xl font-semibold">Secret</h2>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-5 mt-3">

          </div>
        </div>
        <div>
          <h2 class="text-lg sm:text-xl font-semibold">Events</h2>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-5 mt-3">
            {events.map((event) => <label key={event.id} class="">
              <div class="pl-5">
                <input type="checkbox" class="-ml-5 mt-1.5 float-left" name={event.id} />
                <h3 class="sm:text-lg font-semibold inline-block">{event.name}</h3>
                <div>{event.description}</div>
              </div>
            </label>)}
          </div>
        </div>
      </div>
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const [user, data, webhookResp] = await Promise.all([
      ctx.state.userPromise,
      scopeDataWithMember(ctx.state, ctx.params.scope),
      ctx.state.api.get<WebhookEndpoint>(path`/scopes/${ctx.params.scope}/webhooks/${ctx.params.webhook}`),
    ]);
    if (user instanceof Response) return user;
    if (data === null) throw new HttpError(404, "The scope was not found.");

    const iam = scopeIAM(ctx.state, data?.scopeMember, user);
    if (!iam.canAdmin) throw new HttpError(404, "The scope was not found.");

    if (!webhookResp.ok) {
      if (webhookResp.code === "webhookNotFound") {
        throw new HttpError(404, "The webhook was not found.");
      }
      throw webhookResp; // graceful handle errors
    }

    ctx.state.meta = { title: `Settings - @${data.scope.scope} - JSR` };
    return {
      data: {
        scope: data.scope as FullScope,
        webhook: webhookResp.data,
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

export const config: RouteConfig = {
  routeOverride: "/@:scope/~/settings/webhooks/:webhook",
};

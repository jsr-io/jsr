// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError } from "fresh";
import { AccountLayout } from "./(_components)/AccountLayout.tsx";
import { QuotaCard } from "../../components/QuotaCard.tsx";
import { define } from "../../util.ts";
import { TicketModal } from "../../islands/TicketModal.tsx";

export default define.page<typeof handler>(function AccountInvitesPage({
  data,
}) {
  return (
    <AccountLayout user={data.user} active="Settings">
      <div class="flex flex-col gap-12">
        <div>
          <h2 class="text-xl mb-2 font-bold">Quotas</h2>
          <div class="flex flex-col gap-8">
            <div class="flex flex-col justify-between gap-4">
              <div class="max-w-xl">
                <p class="text-jsr-gray-600">
                  Users have certain quotas to help prevent abuse. We are happy
                  to increase your quotas as needed — just send us an increase
                  request.
                </p>
              </div>
              <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                <QuotaCard
                  title="Created scopes"
                  description="The total number of scopes you have created."
                  limit={data.user.scopeLimit}
                  usage={data.user.scopeUsage}
                />
              </div>
            </div>
            <div>
              <TicketModal
                user={data.user}
                style="primary"
                kind="user_scope_quota_increase"
                fields={[{
                  name: "message",
                  label: "Reason",
                  type: "textarea",
                  required: true,
                }]}
              >
                Request user quota increase
              </TicketModal>
            </div>
          </div>
        </div>
        <div>
          <h2 class="text-xl mb-2 font-bold">Delete account</h2>
          <p class="mt-2 text-jsr-gray-600 max-w-xl">
            You may delete your account at any time. If you delete your account,
            any scopes that you are the sole owner of will be orphaned. You will
            not be able to recover your account after deletion.
          </p>
          {/* removing delete button until we offer that functionality */}
          <p class="mt-4 text-red-600">
            Please contact help@jsr.io to delete your account.
          </p>
        </div>
      </div>
    </AccountLayout>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const [currentUser] = await Promise.all([
      ctx.state.userPromise,
    ]);
    if (currentUser instanceof Response) return currentUser;
    if (!currentUser) throw new HttpError(404, "No signed in user found.");

    ctx.state.meta = { title: "Account Settings - JSR" };
    return {
      data: {
        user: currentUser,
      },
    };
  },
});

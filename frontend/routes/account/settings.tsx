// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, HttpError, PageProps } from "@fresh/core";
import { State } from "../../util.ts";
import { FullUser } from "../../utils/api_types.ts";
import { AccountLayout } from "../account/(_components)/AccountLayout.tsx";
import { QuotaCard } from "../../components/QuotaCard.tsx";

interface Data {
  user: FullUser;
}

export default function AccountInvitesPage(
  { data }: PageProps<Data, State>,
) {
  const requestLimitIncreaseBody = `Hello JSR team,
I would like to request a scope quota increase for my account.
My user ID is '${data.user!.id}'.

Reason: `;

  return (
    <AccountLayout user={data.user} active="Settings">
      <div class="flex flex-col gap-12">
        <div>
          <h2 class="text-xl mb-2 font-bold">Quotas</h2>
          <div class="flex flex-col gap-8">
            <div class="flex flex-col justify-between gap-4">
              <div class="max-w-xl">
                <p class="text-gray-600">
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
              <a
                href={`mailto:quotas@jsr.io?subject=${
                  encodeURIComponent(
                    `User quota increase for ${data.user!.name}`,
                  )
                }&body=${encodeURIComponent(requestLimitIncreaseBody)}`}
                class="button-primary"
              >
                Request user quota increase
              </a>
            </div>
          </div>
        </div>
        <div>
          <h2 class="text-xl mb-2 font-bold">Delete account</h2>
          <p class="mt-2 text-gray-600 max-w-xl">
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
}

export const handler: Handlers<Data, State> = {
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
};

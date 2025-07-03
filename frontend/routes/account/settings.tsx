// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError } from "fresh";
import { AccountLayout } from "./(_components)/AccountLayout.tsx";
import { QuotaCard } from "../../components/QuotaCard.tsx";
import { define } from "../../util.ts";
import { TicketModal } from "../../islands/TicketModal.tsx";
import { asset } from "fresh/runtime";

export default define.page<typeof handler>(function AccountInvitesPage({
  data,
}) {
  // @ts-ignore this is possible, typescript just doesnt like it.
  const connectionsCount = !!data.user.githubId + !!data.user.gitlabId;

  return (
    <AccountLayout user={data.user} active="Settings">
      <div class="flex flex-col gap-12">
        <div>
          <h2 class="text-xl mb-2 font-bold">Quotas</h2>
          <div class="flex flex-col gap-8">
            <div class="flex flex-col justify-between gap-4">
              <div class="max-w-xl">
                <p class="text-secondary">
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
                title="Request scope quota increase"
                description={
                  <>
                    <p class="mt-4 text-secondary">
                      We are unable to increase your scope quota without a valid
                      reason, and we require that you make use of your existing
                      scopes before requesting an increase. Please be aware of
                      the{" "}
                      <a
                        href="/docs/usage-policy#scope-name-squatting"
                        class="link"
                      >
                        scope name squatting policy
                      </a>.
                    </p>
                  </>
                }
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
          <h2 class="text-xl mb-2 font-bold">Connected accounts</h2>
          <p class="mt-2 text-secondary max-w-xl">
            You may connect other services at any point, however at least one
            service needs to be connected at any time.
          </p>
          <div class="flex gap-5 mt-4">
            <Connection
              name="GitHub"
              serviceId="github"
              id={data.user.githubId}
              connectionsCount={connectionsCount}
            />
            <Connection
              name="GitLab"
              serviceId="gitlab"
              id={data.user.gitlabId}
              connectionsCount={connectionsCount}
            />
          </div>
        </div>
        <div>
          <h2 class="text-xl mb-2 font-bold">Delete account</h2>
          <p class="mt-2 text-secondary max-w-xl">
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

function Connection(
  { name, serviceId, id, connectionsCount }: {
    name: string;
    serviceId: string;
    id: number | null;
    connectionsCount: number;
  },
) {
  if (connectionsCount === 1 && id !== null) {
    return (
      <button disabled class="button-primary" type="button">
        <img class="size-5" src={asset(`/logos/${serviceId}.svg`)} />
        Disconnect {name}
      </button>
    );
  }

  return (
    <a
      href={`/${id === null ? "" : "dis"}connect/${serviceId}`}
      class="button-primary"
    >
      <img class="size-5" src={asset(`/logos/${serviceId}.svg`)} />
      {id === null ? "Connect" : "Disconnect"} {name}
    </a>
  );
}

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

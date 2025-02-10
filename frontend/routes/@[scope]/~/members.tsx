function MemberLeave(
  props: { userId: string; isAdmin: boolean; isLastAdmin: boolean },
) {
  return (
    <form
      method="POST"
      class="max-w-3xl border-t border-jsr-cyan-950/10 dark:border-jsr-cyan-50/10 pt-8 mt-12"
    >
      <h2 class="text-lg font-semibold">Leave scope</h2>
      <p class="mt-2 text-secondary">
        Leaving this scope will revoke your access to all packages in this
        scope. You will no longer be able to publish packages to this
        scope{props.isAdmin && " or manage members"}.
      </p>
      <input type="hidden" name="userId" value={props.userId} />
      {props.isLastAdmin && (
        <div class="mt-6 border-1 rounded-md border-red-300 bg-red-50 p-6 text-red-600">
          <span class="font-bold text-xl">Warning</span>
          <p>
            You are the last admin in this scope. You must promote another
            member to admin before leaving.
          </p>
        </div>
      )}
      <button
        class="button-danger mt-6"
        type="submit"
        name="action"
        value="deleteMember"
        disabled={props.isLastAdmin}
      >
        Leave
      </button>
    </form>
  );
}

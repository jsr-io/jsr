// Copyright 2024 the JSR authors. All rights reserved. MIT license.

export function DeleteVersionButton(props: { version: string }) {
  function onSubmit(e: Event) {
    if (
      !confirm(
        `Are you sure you want to permanently delete version ${props.version}? This cannot be undone, and the version number cannot be re-published.`,
      )
    ) {
      e.preventDefault();
    }
  }

  return (
    <form method="POST" class="z-20" onSubmit={onSubmit}>
      <input type="hidden" name="version" value={props.version} />
      <button
        type="submit"
        class="button-danger"
        name="action"
        value="delete"
      >
        Delete
      </button>
    </form>
  );
}

import { api, path } from "../../../../utils/api.ts";

export function RevokeToken(props: { id: string }) {
  function onClick(e: Event) {
    e.preventDefault();
    if (!confirm("Are you sure you want to revoke this token?")) {
      return;
    }

    api.delete(path`/user/tokens/${props.id}`).then((res) => {
      if (res.ok) {
        location.reload();
      } else {
        console.error(res);
        alert("Failed to revoke token");
      }
    });
  }

  return (
    <button class="text-red-500 underline hover:text-red-700" onClick={onClick}>
      Revoke token
    </button>
  );
}

import { signOut } from "next-auth/react";
import { useState } from "react";

import Layout from "@/components/Layout";
import { useMe } from "@/components/MeContext";
import { Avatar, ErrorMessage, LoadingBlock, PageHeader } from "@/components/ui";
import { ApiError } from "@/lib/api-client";

export default function Account() {
  const { me, loading } = useMe();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>();

  async function disconnect() {
    if (
      !window.confirm(
        "Disconnect your GitHub account? This deletes your tracked repositories and cached statistics from GitCore and revokes its access on GitHub. Nothing on GitHub itself is changed.",
      )
    ) {
      return;
    }
    setBusy(true);
    setError(undefined);
    try {
      const res = await fetch("/api/account/disconnect", { method: "POST", credentials: "same-origin" });
      const json = await res.json().catch(() => null);
      if (!res.ok) {
        throw new ApiError(res.status, json?.error?.code ?? "http_error", json?.error?.message ?? "Disconnect failed.");
      }
      await signOut({ callbackUrl: "/login?disconnected=1" });
    } catch (err) {
      setError(err);
      setBusy(false);
    }
  }

  return (
    <Layout title="Account">
      <PageHeader title="Account" />
      {loading && !me ? <LoadingBlock /> : null}
      {me ? (
        <div className="max-w-2xl space-y-6">
          <section className="card flex items-center gap-4">
            <Avatar src={me.user.avatar_url} alt="" size={56} />
            <div>
              <div className="text-lg font-semibold">{me.user.name || me.user.login}</div>
              <a href={me.user.html_url} target="_blank" rel="noopener noreferrer" className="text-sm text-sky-700 hover:underline">
                @{me.user.login} on GitHub ↗
              </a>
            </div>
          </section>

          <section className="card space-y-3">
            <h2 className="text-lg font-semibold">GitHub connection</h2>
            <p className="text-sm text-gray-600">
              GitCore acts on GitHub with your permissions. Disconnecting deletes everything GitCore stored for you
              (tracked repositories and synced issue statistics), revokes GitCore&apos;s OAuth access, and signs you out.
            </p>
            {error ? <ErrorMessage error={error} /> : null}
            <button className="btn-danger" onClick={disconnect} disabled={busy}>
              {busy ? "Disconnecting…" : "Disconnect GitHub account"}
            </button>
          </section>
        </div>
      ) : null}
    </Layout>
  );
}

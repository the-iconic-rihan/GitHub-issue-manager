import Head from "next/head";
import Image from "next/image";
import { useRouter } from "next/router";
import { signIn, useSession } from "next-auth/react";
import { useEffect, useState } from "react";

import { Notice } from "@/components/ui";
import { safeCallbackUrl } from "@/lib/format";

const ERRORS: Record<string, string> = {
  SessionExpired: "Your GitHub session has expired or was revoked. Please sign in again.",
  AccessDenied: "GitHub access was denied. Authorize the app to continue.",
  OAuthSignin: "Could not start GitHub sign-in. Check the OAuth configuration.",
  OAuthCallback: "GitHub sign-in failed. Please try again.",
  Configuration: "Sign-in is not configured correctly on the server.",
};

export default function Login() {
  const router = useRouter();
  const { status } = useSession();
  const [pending, setPending] = useState(false);
  const callbackUrl = safeCallbackUrl(router.query.callbackUrl);
  const errorKey = typeof router.query.error === "string" ? router.query.error : undefined;
  const disconnected = router.query.disconnected === "1";

  useEffect(() => {
    if (status === "authenticated") void router.replace(callbackUrl);
  }, [status, router, callbackUrl]);

  async function handleGithubSignIn() {
    setPending(true);
    await signIn("github", { callbackUrl });
  }

  return (
    <>
      <Head>
        <title>Sign in - GitCore</title>
      </Head>
      <div className="flex min-h-screen items-center justify-center bg-gray-100 px-4 py-2">
        <main className="flex w-full flex-1 flex-col items-center justify-center text-center">
          <div className="w-full max-w-md rounded-2xl bg-white shadow-2xl">
            <div className="w-full p-5">
              <div className="text-center font-bold text-black">
                <span className="text-sky-500">Git</span>Core
              </div>
              <div className="py-10">
                <h1 className="mb-3 text-3xl font-bold text-black">Sign in</h1>
                <div className="mb-3 inline-block w-10 rounded-xl border-2 border-sky-500 bg-sky-500" />
                <p className="mx-auto mb-6 max-w-xs text-sm text-gray-500">
                  Manage issues across your GitHub repositories in one place.
                </p>
                <div className="mx-auto mb-4 max-w-xs space-y-2 text-left">
                  {errorKey ? <Notice kind="error">{ERRORS[errorKey] ?? "Sign-in failed. Please try again."}</Notice> : null}
                  {disconnected ? (
                    <Notice kind="success">Your GitHub account was disconnected and your data deleted.</Notice>
                  ) : null}
                </div>
                <div className="my-3 flex flex-col items-center justify-center">
                  <button
                    type="button"
                    onClick={handleGithubSignIn}
                    disabled={pending || status === "loading"}
                    className="m-2 flex w-64 items-center justify-center rounded-full border-2 border-gray-200 p-3 font-medium transition hover:border-sky-500 disabled:opacity-60"
                  >
                    <span className="mx-2">{pending ? "Redirecting to GitHub…" : "Sign in with GitHub"}</span>
                    <Image src="/assests/github.svg" alt="" width={25} height={25} />
                  </button>
                </div>
                <p className="mx-auto mt-4 max-w-xs text-xs text-gray-400">
                  GitCore requests access to your repositories so it can read and update issues on your behalf.
                </p>
              </div>
            </div>
          </div>
        </main>
      </div>
    </>
  );
}

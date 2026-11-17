import "../styles/globals.css";

import type { AppProps } from "next/app";
import { useRouter } from "next/router";
import type { Session } from "next-auth";
import { SessionProvider, useSession } from "next-auth/react";
import { useEffect, type ReactNode } from "react";

import { MeProvider } from "@/components/MeContext";
import { LoadingBlock } from "@/components/ui";

/** Pages reachable without signing in. */
const PUBLIC_PAGES = new Set(["/login", "/404", "/_error"]);

function AuthGuard({ children }: { children: ReactNode }) {
  const router = useRouter();
  const { status } = useSession();

  useEffect(() => {
    if (status === "unauthenticated") {
      void router.replace({ pathname: "/login", query: { callbackUrl: router.asPath } });
    }
  }, [status, router]);

  if (status !== "authenticated") {
    return (
      <div className="min-h-screen bg-gray-100">
        <LoadingBlock label="Checking your session" />
      </div>
    );
  }
  return <MeProvider>{children}</MeProvider>;
}

export default function App({ Component, pageProps, router }: AppProps<{ session?: Session }>) {
  const { session, ...rest } = pageProps;
  const page = <Component {...rest} />;
  return (
    <SessionProvider session={session}>
      {PUBLIC_PAGES.has(router.pathname) ? page : <AuthGuard>{page}</AuthGuard>}
    </SessionProvider>
  );
}

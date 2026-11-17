import type { NextAuthOptions } from "next-auth";
import GithubProvider, { type GithubProfile } from "next-auth/providers/github";

import { githubApiUrl, githubScopes, githubWebUrl } from "./config";

const SESSION_MAX_AGE = 7 * 24 * 60 * 60;

/**
 * Sign in with GitHub (OAuth App). The access token is stored only inside the encrypted
 * session JWT cookie (HttpOnly) and read server-side by the API proxy; the `session`
 * object sent to the browser never contains it.
 */
export function authOptions(): NextAuthOptions {
  const web = githubWebUrl();
  const api = githubApiUrl();

  return {
    providers: [
      GithubProvider({
        clientId: process.env.GITHUB_ID ?? "",
        clientSecret: process.env.GITHUB_SECRET ?? "",
        authorization: { url: `${web}/login/oauth/authorize`, params: { scope: githubScopes() } },
        token: `${web}/login/oauth/access_token`,
        userinfo: {
          url: `${api}/user`,
          async request({ tokens }) {
            const headers = {
              Authorization: `Bearer ${tokens.access_token}`,
              Accept: "application/vnd.github+json",
              "User-Agent": "github-issue-manager",
            };
            const res = await fetch(`${api}/user`, { headers });
            if (!res.ok) throw new Error(`GitHub user lookup failed (HTTP ${res.status})`);
            const profile = (await res.json()) as GithubProfile;
            if (!profile.email) {
              // Private emails are only available from /user/emails; optional.
              const emails = await fetch(`${api}/user/emails`, { headers }).catch(() => null);
              if (emails?.ok) {
                const list = (await emails.json()) as { email: string; primary: boolean }[];
                profile.email = (list.find((e) => e.primary) ?? list[0])?.email ?? null;
              }
            }
            // next-auth's Profile type uses `undefined` (not `null`) for missing values.
            return { ...profile, name: profile.name ?? undefined, email: profile.email ?? undefined };
          },
        },
      }),
    ],
    session: { strategy: "jwt", maxAge: SESSION_MAX_AGE },
    jwt: { maxAge: SESSION_MAX_AGE },
    pages: { signIn: "/login", error: "/login" },
    callbacks: {
      async jwt({ token, account, profile }) {
        if (account?.access_token) {
          token.accessToken = account.access_token;
          const p = profile as GithubProfile | undefined;
          token.githubId = p?.id;
          token.login = p?.login;
        }
        return token;
      },
      async session({ session, token }) {
        if (session.user) {
          session.user.login = token.login;
          session.user.githubId = token.githubId;
        }
        return session;
      },
    },
  };
}

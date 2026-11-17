import "next-auth";
import "next-auth/jwt";

declare module "next-auth" {
  interface Session {
    user?: {
      name?: string | null;
      email?: string | null;
      image?: string | null;
      login?: string;
      githubId?: number;
    };
  }
}

declare module "next-auth/jwt" {
  interface JWT {
    /** GitHub OAuth token; server-side only. */
    accessToken?: string;
    githubId?: number;
    login?: string;
  }
}

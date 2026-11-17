import NextAuth from "next-auth";

import { authOptions } from "@/lib/server/auth-options";

export default NextAuth(authOptions());

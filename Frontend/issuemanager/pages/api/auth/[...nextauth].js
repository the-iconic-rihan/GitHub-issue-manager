// [...nextauth].js  is saved in such a way to catch all the routes
import NextAuth from "next-auth";
import GoogleProvider from "next-auth/providers/google";
// import GithubProvider from "next-auth/providers/github"

export default NextAuth({
  providers: [
    GoogleProvider({
      clientId: process.env.GOOGLE_ID,
      clientSecret: process.env.GOOGLE_SECRET
    }),
  ],
});

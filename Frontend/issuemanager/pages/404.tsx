import Head from "next/head";
import Link from "next/link";

export default function NotFound() {
  return (
    <>
      <Head>
        <title>Not found - GitCore</title>
      </Head>
      <div className="flex min-h-screen items-center justify-center bg-gray-100 px-4">
        <div className="card max-w-md text-center">
          <h1 className="text-2xl font-bold">Page not found</h1>
          <p className="mt-2 text-sm text-gray-500">The page you are looking for does not exist.</p>
          <Link href="/" className="btn-primary mt-6">
            Go to dashboard
          </Link>
        </div>
      </div>
    </>
  );
}

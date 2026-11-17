import Head from "next/head";
import Link from "next/link";
import { useRouter } from "next/router";
import { signOut, useSession } from "next-auth/react";
import { useState, type ReactNode } from "react";
import { FaBars, FaTimes } from "react-icons/fa";

import { useMe } from "./MeContext";
import { Avatar, ErrorMessage } from "./ui";

function NavLink({ href, children, onClick }: { href: string; children: ReactNode; onClick?: () => void }) {
  const router = useRouter();
  const active = href === "/" ? router.pathname === "/" : router.pathname.startsWith(href);
  return (
    <Link
      href={href}
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={`rounded-xl px-3 py-2 text-sm font-medium ${active ? "bg-sky-100 text-sky-900" : "text-gray-700 hover:bg-gray-100"}`}
    >
      {children}
    </Link>
  );
}

export default function Layout({ title, children }: { title: string; children: ReactNode }) {
  const { data: session } = useSession();
  const router = useRouter();
  const [menuOpen, setMenuOpen] = useState(false);
  const meState = useMe();

  // Close the mobile menu whenever the route changes (including back/forward).
  const [menuPath, setMenuPath] = useState(router.asPath);
  if (menuPath !== router.asPath) {
    setMenuPath(router.asPath);
    setMenuOpen(false);
  }

  const me = meState.me;
  const discoverEnabled = me?.discover_enabled ?? false;
  const closeMenu = () => setMenuOpen(false);
  const links = (
    <>
      <NavLink href="/" onClick={closeMenu}>Dashboard</NavLink>
      <NavLink href="/repos" onClick={closeMenu}>Repositories</NavLink>
      {discoverEnabled ? <NavLink href="/discover" onClick={closeMenu}>Discover</NavLink> : null}
      <NavLink href="/account" onClick={closeMenu}>Account</NavLink>
    </>
  );

  return (
    <>
      <Head>
        <title>{`${title} - GitCore`}</title>
      </Head>
      <div className="min-h-screen bg-gray-100">
        <header className="bg-white shadow-sm">
          <div className="mx-auto flex max-w-6xl items-center justify-between gap-4 px-4 py-3">
            <Link href="/" className="text-lg font-bold text-black">
              <span className="text-sky-500">Git</span>Core
            </Link>
            <nav className="hidden items-center gap-1 md:flex" aria-label="Main">
              {links}
            </nav>
            <div className="flex items-center gap-3">
              <span className="hidden items-center gap-2 text-sm text-gray-700 sm:flex">
                <Avatar src={me?.user.avatar_url ?? session?.user?.image} alt="" size={28} />
                {me?.user.login ?? session?.user?.login}
              </span>
              <button className="btn-secondary hidden md:inline-flex" onClick={() => signOut({ callbackUrl: "/login" })}>
                Sign out
              </button>
              <button
                className="btn-secondary md:hidden"
                onClick={() => setMenuOpen((o) => !o)}
                aria-expanded={menuOpen}
                aria-controls="mobile-menu"
                aria-label={menuOpen ? "Close menu" : "Open menu"}
              >
                {menuOpen ? <FaTimes /> : <FaBars />}
              </button>
            </div>
          </div>
          {menuOpen ? (
            <nav id="mobile-menu" className="flex flex-col gap-1 border-t px-4 py-3 md:hidden" aria-label="Main">
              {links}
              <button className="btn-secondary mt-2" onClick={() => signOut({ callbackUrl: "/login" })}>
                Sign out
              </button>
            </nav>
          ) : null}
        </header>
        <main className="mx-auto max-w-6xl px-4 py-6">
          {meState.error ? (
            <div className="mb-6">
              <ErrorMessage error={meState.error} onRetry={meState.reload} />
            </div>
          ) : null}
          {children}
        </main>
      </div>
    </>
  );
}

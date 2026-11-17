import { signOut } from "next-auth/react";
import type { ReactNode } from "react";
import { FaCheckCircle, FaDotCircle, FaExclamationTriangle } from "react-icons/fa";

import { ApiError, errorMessage } from "@/lib/api-client";
import { labelColors } from "@/lib/format";

export function Spinner({ label = "Loading" }: { label?: string }) {
  return (
    <span role="status" className="inline-flex items-center gap-2 text-sm text-gray-500">
      <span className="h-4 w-4 animate-spin rounded-full border-2 border-sky-400 border-t-transparent" aria-hidden />
      <span>{label}…</span>
    </span>
  );
}

export function LoadingBlock({ label }: { label?: string }) {
  return (
    <div className="flex justify-center py-12">
      <Spinner label={label} />
    </div>
  );
}

export function ErrorMessage({ error, onRetry }: { error: unknown; onRetry?: () => void }) {
  const apiError = error instanceof ApiError ? error : undefined;
  return (
    <div role="alert" className="rounded-xl border border-red-200 bg-red-50 p-4 text-sm text-red-800">
      <div className="flex items-start gap-2">
        <FaExclamationTriangle className="mt-0.5 shrink-0" aria-hidden />
        <div className="flex-1">
          <p>{errorMessage(error)}</p>
          {apiError?.retryAfter ? (
            <p className="mt-1 text-red-700">GitHub asks us to wait about {apiError.retryAfter} seconds.</p>
          ) : null}
          <div className="mt-3 flex flex-wrap gap-2">
            {apiError?.isUnauthorized ? (
              <button className="btn-secondary" onClick={() => signOut({ callbackUrl: "/login?error=SessionExpired" })}>
                Sign in again
              </button>
            ) : onRetry ? (
              <button className="btn-secondary" onClick={onRetry}>
                Try again
              </button>
            ) : null}
          </div>
        </div>
      </div>
    </div>
  );
}

export function EmptyState({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="rounded-2xl border-2 border-dashed border-gray-200 bg-white p-10 text-center">
      <h3 className="text-lg font-semibold text-gray-800">{title}</h3>
      {children ? <div className="mt-2 text-sm text-gray-500">{children}</div> : null}
    </div>
  );
}

export type NoticeKind = "success" | "error" | "info";

export function Notice({ kind, children, onDismiss }: { kind: NoticeKind; children: ReactNode; onDismiss?: () => void }) {
  const styles: Record<NoticeKind, string> = {
    success: "border-green-200 bg-green-50 text-green-800",
    error: "border-red-200 bg-red-50 text-red-800",
    info: "border-sky-200 bg-sky-50 text-sky-900",
  };
  return (
    <div role={kind === "error" ? "alert" : "status"} className={`flex items-start gap-3 rounded-xl border p-3 text-sm ${styles[kind]}`}>
      <div className="flex-1">{children}</div>
      {onDismiss ? (
        <button className="text-xs font-semibold underline" onClick={onDismiss} aria-label="Dismiss">
          Dismiss
        </button>
      ) : null}
    </div>
  );
}

export function Avatar({ src, alt, size = 24 }: { src?: string | null; alt: string; size?: number }) {
  if (!src) return null;
  return (
    // Remote GitHub avatars; the image optimizer is disabled (see next.config.js).
    // eslint-disable-next-line @next/next/no-img-element
    <img src={src} alt={alt} width={size} height={size} className="inline-block rounded-full bg-gray-200" style={{ width: size, height: size }} />
  );
}

export function StateBadge({ state, reason }: { state: string; reason?: string | null }) {
  const open = state === "open";
  const label = open ? "Open" : reason === "not_planned" ? "Closed (not planned)" : "Closed";
  return (
    <span
      className={`inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-semibold ${
        open ? "bg-green-100 text-green-800" : "bg-purple-100 text-purple-800"
      }`}
    >
      {open ? <FaDotCircle aria-hidden /> : <FaCheckCircle aria-hidden />}
      {label}
    </span>
  );
}

export function LabelBadge({ name, color, onRemove }: { name: string; color?: string; onRemove?: () => void }) {
  const style = color ? labelColors(color) : { background: "#e5e7eb", color: "#111827" };
  return (
    <span className="inline-flex max-w-full items-center gap-1 truncate rounded-full px-2 py-0.5 text-xs font-medium" style={style}>
      <span className="truncate">{name}</span>
      {onRemove ? (
        <button type="button" onClick={onRemove} aria-label={`Remove label ${name}`} className="ml-0.5 font-bold opacity-80 hover:opacity-100">
          ×
        </button>
      ) : null}
    </span>
  );
}

export function PageHeader({ title, subtitle, actions }: { title: ReactNode; subtitle?: ReactNode; actions?: ReactNode }) {
  return (
    <div className="mb-6 flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
      <div className="min-w-0">
        <h1 className="break-words text-2xl font-bold text-gray-900">{title}</h1>
        {subtitle ? <div className="mt-1 text-sm text-gray-500">{subtitle}</div> : null}
      </div>
      {actions ? <div className="flex flex-wrap gap-2">{actions}</div> : null}
    </div>
  );
}

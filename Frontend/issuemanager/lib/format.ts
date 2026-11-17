const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ["year", 365 * 24 * 3600],
  ["month", 30 * 24 * 3600],
  ["week", 7 * 24 * 3600],
  ["day", 24 * 3600],
  ["hour", 3600],
  ["minute", 60],
];

/** "3 days ago" style relative time. */
export function relativeTime(iso: string | null | undefined, now: Date = new Date()): string {
  if (!iso) return "never";
  const then = new Date(iso);
  if (Number.isNaN(then.getTime())) return "unknown";
  const seconds = Math.round((then.getTime() - now.getTime()) / 1000);
  const rtf = new Intl.RelativeTimeFormat("en", { numeric: "auto" });
  for (const [unit, size] of UNITS) {
    if (Math.abs(seconds) >= size) return rtf.format(Math.round(seconds / size), unit);
  }
  return "just now";
}

const HEX = /^[0-9a-fA-F]{6}$/;

/** Background/foreground colours for a GitHub label colour, with readable contrast. */
export function labelColors(color: string): { background: string; color: string } {
  const hex = HEX.test(color) ? color : "e5e7eb";
  const r = parseInt(hex.slice(0, 2), 16);
  const g = parseInt(hex.slice(2, 4), 16);
  const b = parseInt(hex.slice(4, 6), 16);
  // Perceived luminance (ITU-R BT.601).
  const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  return { background: `#${hex}`, color: luminance > 0.6 ? "#111827" : "#ffffff" };
}

/** Only same-site relative paths are accepted as post-login destinations (no open redirects). */
export function safeCallbackUrl(value: unknown): string {
  return typeof value === "string" && value.startsWith("/") && !value.startsWith("//") && !value.startsWith("/\\")
    ? value
    : "/";
}

/** Splits "owner/repo" safely. */
export function splitFullName(fullName: string): { owner: string; repo: string } {
  const [owner = "", repo = ""] = fullName.split("/", 2);
  return { owner, repo };
}

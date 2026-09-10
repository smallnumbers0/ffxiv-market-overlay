/** Display helpers. Pure functions - no React, no Tauri. */

const GIL = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 });

/** `12,345` - gil is always a whole number in the UI. */
export const gil = (value: number | null | undefined): string =>
  value === null || value === undefined ? "-" : GIL.format(Math.round(value));

/** Compact counts for the settings panel: `16,845`. */
export const count = (value: number): string => GIL.format(value);

/** `2.4/day`, or `-` when Universalis reports no movement. */
export const velocity = (value: number): string =>
  value > 0 ? `${value.toFixed(1)}/day` : "-";

/**
 * "just now" / "4m ago" / "3h ago" / "2d ago".
 * Takes Unix milliseconds; `null` renders as a dash.
 */
export function timeAgo(
  millis: number | null | undefined,
  now = Date.now(),
): string {
  if (millis === null || millis === undefined || millis <= 0) return "-";
  const seconds = Math.max(0, Math.round((now - millis) / 1000));
  if (seconds < 45) return "just now";
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.round(hours / 24)}d ago`;
}

/** Universalis reports sale timestamps in seconds, not milliseconds. */
export const timeAgoSeconds = (seconds: number, now = Date.now()): string =>
  timeAgo(seconds * 1000, now);

/** `last_synced_at` is stored as epoch millis in a TEXT column. */
export function syncedAt(value: string | null): string {
  if (!value) return "never";
  const millis = Number(value);
  return Number.isFinite(millis) && millis > 0
    ? new Date(millis).toLocaleString()
    : "never";
}

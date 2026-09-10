/**
 * Best-effort guess at which Universalis region a user plays on, from the
 * system time zone.
 *
 * This exists so the app shows real prices the moment it opens instead of
 * demanding a setup step first. It is only ever a starting point: the chosen
 * scope is always visible in the title bar, and the UI nudges the user to
 * narrow it to their own world, which is the only way to get prices they can
 * actually act on without travelling.
 *
 * Region names must match Universalis's `/api/v2/data-centers` `region` values
 * exactly, since they are used directly as a query scope.
 */

export const DEFAULT_REGION = "North-America";

/** Time-zone area prefix -> region. Checked before the exact-zone table. */
const AREA_REGIONS: Record<string, string> = {
  America: "North-America",
  Europe: "Europe",
  Africa: "Europe",
  Atlantic: "Europe",
  Australia: "Oceania",
};

/** Zones whose area prefix is ambiguous (Asia and Pacific mostly). */
const ZONE_REGIONS: Record<string, string> = {
  "Asia/Tokyo": "Japan",
  "Asia/Seoul": "한국",
  "Asia/Pyongyang": "한국",
  "Asia/Shanghai": "中国",
  "Asia/Chongqing": "中国",
  "Asia/Harbin": "中国",
  "Asia/Urumqi": "中国",
  "Asia/Macau": "中国",
  "Asia/Hong_Kong": "中国",
  "Asia/Taipei": "繁中服",
  "Pacific/Auckland": "Oceania",
  "Pacific/Chatham": "Oceania",
  "Pacific/Honolulu": "North-America",
};

/**
 * Map an IANA time zone to a Universalis region. Falls back to
 * `DEFAULT_REGION` for anywhere unmapped - a wrong guess is recoverable in one
 * click, an empty app is not.
 */
export function regionForTimeZone(timeZone: string | undefined): string {
  if (!timeZone) return DEFAULT_REGION;
  if (ZONE_REGIONS[timeZone]) return ZONE_REGIONS[timeZone];

  const area = timeZone.split("/")[0];
  return AREA_REGIONS[area] ?? DEFAULT_REGION;
}

/** The region for whatever time zone this machine is set to. */
export function guessRegion(): string {
  try {
    return regionForTimeZone(Intl.DateTimeFormat().resolvedOptions().timeZone);
  } catch {
    // Intl is always present in WebView2, but never let a guess break startup.
    return DEFAULT_REGION;
  }
}

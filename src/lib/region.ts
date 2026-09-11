/**
 * Best-effort guess at which Universalis region a user plays on, from the
 * system time zone.
 *
 * This exists so the app shows real prices the moment it opens instead of
 * demanding a setup step first. It is only ever a starting point, and it says
 * so by being ordinary: the board strip names the scope like any other, and
 * clicking it opens the picker. Nothing announces the guess - a banner every
 * user has to dismiss costs more than the guess being wrong.
 *
 * A region is never queried directly - whole-region requests are slow enough
 * to time out. The backend resolves the guess to one data center inside the
 * region and boards that instead. Region names must still match Universalis's
 * `/api/v2/data-centers` `region` values exactly, since that resolution and
 * the picker's grouping both match on them.
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

/**
 * Order two region names for the world picker, floating the player's own
 * region to the top and leaving the rest alphabetical.
 *
 * Alphabetical on its own buries North-America behind Europe and Japan, which
 * is a lot of scrolling past worlds you will never price-check. The home
 * region comes from the same time-zone guess that seeds the first board, so
 * the picker opens on the servers the player actually plays on.
 */
export function compareRegions(a: string, b: string, homeRegion: string): number {
  if (a === b) return 0;
  if (a === homeRegion) return -1;
  if (b === homeRegion) return 1;
  return a.localeCompare(b);
}

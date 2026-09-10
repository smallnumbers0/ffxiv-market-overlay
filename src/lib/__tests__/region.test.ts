import { describe, expect, it } from "vitest";

import { DEFAULT_REGION, regionForTimeZone } from "../region";

describe("regionForTimeZone", () => {
  it("maps the common play regions", () => {
    expect(regionForTimeZone("America/New_York")).toBe("North-America");
    expect(regionForTimeZone("America/Los_Angeles")).toBe("North-America");
    expect(regionForTimeZone("Europe/London")).toBe("Europe");
    expect(regionForTimeZone("Europe/Berlin")).toBe("Europe");
    expect(regionForTimeZone("Asia/Tokyo")).toBe("Japan");
    expect(regionForTimeZone("Australia/Sydney")).toBe("Oceania");
  });

  it("prefers the exact-zone table over the area prefix", () => {
    // Both live under "Asia/" and "Pacific/", which say nothing on their own.
    expect(regionForTimeZone("Asia/Seoul")).toBe("한국");
    expect(regionForTimeZone("Asia/Shanghai")).toBe("中国");
    expect(regionForTimeZone("Pacific/Auckland")).toBe("Oceania");
    expect(regionForTimeZone("Pacific/Honolulu")).toBe("North-America");
  });

  it("falls back rather than leaving the app with no scope", () => {
    expect(regionForTimeZone("Antarctica/Casey")).toBe(DEFAULT_REGION);
    expect(regionForTimeZone("Not/A/Zone")).toBe(DEFAULT_REGION);
    expect(regionForTimeZone("")).toBe(DEFAULT_REGION);
    expect(regionForTimeZone(undefined)).toBe(DEFAULT_REGION);
  });
});

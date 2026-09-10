import { describe, expect, it } from "vitest";

import { gil, syncedAt, timeAgo, timeAgoSeconds, velocity } from "../format";

describe("gil", () => {
  it("groups thousands and rounds", () => {
    expect(gil(1234567)).toBe("1,234,567");
    expect(gil(215.6)).toBe("216");
  });

  it("renders absent values as a dash, but keeps a real zero distinct", () => {
    expect(gil(null)).toBe("-");
    expect(gil(undefined)).toBe("-");
    expect(gil(0)).toBe("0");
  });
});

describe("velocity", () => {
  it("shows one decimal, and a dash when nothing is moving", () => {
    expect(velocity(79.26)).toBe("79.3/day");
    expect(velocity(0)).toBe("-");
  });
});

describe("timeAgo", () => {
  const now = Date.UTC(2026, 0, 2, 12, 0, 0);

  it("scales the unit to the age", () => {
    expect(timeAgo(now - 10_000, now)).toBe("just now");
    expect(timeAgo(now - 5 * 60_000, now)).toBe("5m ago");
    expect(timeAgo(now - 3 * 3_600_000, now)).toBe("3h ago");
    expect(timeAgo(now - 2 * 86_400_000, now)).toBe("2d ago");
  });

  it("treats missing and zero timestamps as unknown", () => {
    expect(timeAgo(null, now)).toBe("-");
    expect(timeAgo(0, now)).toBe("-");
  });

  it("reads Universalis sale timestamps as seconds", () => {
    expect(timeAgoSeconds((now - 3_600_000) / 1000, now)).toBe("1h ago");
  });
});

describe("syncedAt", () => {
  it("reports never for an unsynced catalog", () => {
    expect(syncedAt(null)).toBe("never");
    expect(syncedAt("")).toBe("never");
    expect(syncedAt("not-a-number")).toBe("never");
  });

  it("formats stored epoch millis", () => {
    expect(syncedAt(String(Date.UTC(2026, 0, 2)))).not.toBe("never");
  });
});

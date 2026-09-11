import { describe, expect, it } from "vitest";

import { groupScopes } from "../scopes";
import type { MarketScopes } from "../tauriApi";

/** A trimmed stand-in for Universalis's lists: a few worlds per data center,
 *  deliberately out of alphabetical order and with regions interleaved. */
const scopes: MarketScopes = {
  worlds: [
    { id: 40, name: "Gilgamesh" },
    { id: 41, name: "Adamantoise" },
    { id: 42, name: "Cactuar" },
    { id: 43, name: "Behemoth" },
    { id: 50, name: "Cerberus" },
    { id: 51, name: "Louisoix" },
    { id: 60, name: "Aegis" },
    { id: 61, name: "Atomos" },
    { id: 70, name: "Bismarck" },
  ],
  dataCenters: [
    { name: "Light", region: "Europe", worlds: [50, 51] },
    { name: "Elemental", region: "Japan", worlds: [60, 61] },
    { name: "Primal", region: "North-America", worlds: [40, 43] },
    { name: "Aether", region: "North-America", worlds: [41, 42] },
    { name: "Materia", region: "Oceania", worlds: [70] },
  ],
};

const regionsOf = (home: string) =>
  groupScopes(scopes, home).map((group) => group.region);

describe("groupScopes", () => {
  it("puts the player's own region first", () => {
    expect(regionsOf("North-America")).toEqual([
      "North-America",
      "Europe",
      "Japan",
      "Oceania",
    ]);
  });

  it("does the same for a player anywhere else", () => {
    expect(regionsOf("Japan")).toEqual([
      "Japan",
      "Europe",
      "North-America",
      "Oceania",
    ]);
  });

  it("stays alphabetical when the home region is not in the list", () => {
    expect(regionsOf("한국")).toEqual([
      "Europe",
      "Japan",
      "North-America",
      "Oceania",
    ]);
  });

  it("sorts data centers and their worlds by name", () => {
    const [northAmerica] = groupScopes(scopes, "North-America");
    expect(northAmerica.dataCenters).toEqual([
      { name: "Aether", worldNames: ["Adamantoise", "Cactuar"] },
      { name: "Primal", worldNames: ["Behemoth", "Gilgamesh"] },
    ]);
  });

  it("drops data centers whose worlds are all unknown", () => {
    const withGhost: MarketScopes = {
      ...scopes,
      dataCenters: [
        ...scopes.dataCenters,
        { name: "Shadow", region: "Europe", worlds: [999] },
      ],
    };
    const europe = groupScopes(withGhost, "North-America").find(
      (group) => group.region === "Europe",
    );
    expect(europe?.dataCenters.map((dc) => dc.name)).toEqual(["Light"]);
  });

  it("has nothing to show before the world list loads", () => {
    expect(groupScopes(null, "North-America")).toEqual([]);
  });
});

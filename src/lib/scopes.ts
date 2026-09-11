/**
 * Shaping the flat Universalis world/data-center lists into the tree the
 * settings picker draws: region -> data center -> worlds.
 *
 * Kept out of the component so the ordering rules - which decide how far a
 * player has to scroll to find their own server - can be tested directly.
 */

import { compareRegions } from "./region";
import type { MarketScopes } from "./tauriApi";

export interface ScopeDataCenter {
  name: string;
  worldNames: string[];
}

export interface ScopeRegion {
  region: string;
  dataCenters: ScopeDataCenter[];
}

/**
 * Group worlds under their data center and region, with `homeRegion` first.
 *
 * Data centers with no known worlds are dropped rather than rendered as empty
 * groups - Universalis lists a few placeholder ones.
 */
export function groupScopes(
  scopes: MarketScopes | null,
  homeRegion: string,
): ScopeRegion[] {
  if (!scopes) return [];

  const worldName = new Map(scopes.worlds.map((world) => [world.id, world.name]));
  const regions = new Map<string, ScopeRegion>();

  for (const dc of scopes.dataCenters) {
    const worldNames = dc.worlds
      .map((id) => worldName.get(id))
      .filter((name): name is string => Boolean(name))
      .sort((a, b) => a.localeCompare(b));
    if (worldNames.length === 0) continue;

    let group = regions.get(dc.region);
    if (!group) {
      group = { region: dc.region, dataCenters: [] };
      regions.set(dc.region, group);
    }
    group.dataCenters.push({ name: dc.name, worldNames });
  }

  for (const group of regions.values()) {
    group.dataCenters.sort((a, b) => a.name.localeCompare(b.name));
  }

  return [...regions.values()].sort((a, b) =>
    compareRegions(a.region, b.region, homeRegion),
  );
}

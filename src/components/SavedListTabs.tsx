interface SavedListTabsProps {
  active: "favorites" | "recent";
  onSelect: (list: "favorites" | "recent") => void;
  favoriteCount: number;
  recentCount: number;
}

/**
 * Picks which saved list the empty search box shows.
 *
 * Both tabs are always offered, even when empty: a Favorites tab that only
 * appeared once you had favorites would hide the one place that explains how
 * to make one.
 */
export function SavedListTabs({
  active,
  onSelect,
  favoriteCount,
  recentCount,
}: SavedListTabsProps) {
  return (
    <div className="saved-tabs" role="tablist" aria-label="Saved items">
      <Tab
        label="Favorites"
        count={favoriteCount}
        active={active === "favorites"}
        onSelect={() => onSelect("favorites")}
      />
      <Tab
        label="Recent"
        count={recentCount}
        active={active === "recent"}
        onSelect={() => onSelect("recent")}
      />
    </div>
  );
}

function Tab({
  label,
  count,
  active,
  onSelect,
}: {
  label: string;
  count: number;
  active: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      className={`saved-tab${active ? " is-active" : ""}`}
      onClick={onSelect}
    >
      {label}
      {count > 0 && <span className="saved-tab-count">{count}</span>}
    </button>
  );
}

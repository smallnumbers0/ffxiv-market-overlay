import { useEffect, useRef } from "react";

import { HeartIcon } from "./icons";
import type { Item, SearchResult } from "../lib/tauriApi";
import { iconUrl } from "../lib/tauriApi";

interface ResultsListProps {
  results: SearchResult[] | Item[];
  highlightIndex: number;
  onHighlight: (index: number) => void;
  onSelect: (item: Item) => void;
  emptyMessage?: string;
  /** Hearted ids, for drawing each row's heart in the right state. */
  favorites: number[];
  onToggleFavorite: (itemId: number) => void;
}

export function ResultsList({
  results,
  highlightIndex,
  onHighlight,
  onSelect,
  emptyMessage,
  favorites,
  onToggleFavorite,
}: ResultsListProps) {
  if (results.length === 0) {
    return emptyMessage ? <p className="empty">{emptyMessage}</p> : null;
  }

  return (
    <ul className="results" role="listbox">
      {results.map((item, index) => (
        <ResultRow
          key={item.itemId}
          item={item}
          highlighted={index === highlightIndex}
          favorite={favorites.includes(item.itemId)}
          onHighlight={() => onHighlight(index)}
          onSelect={() => onSelect(item)}
          onToggleFavorite={() => onToggleFavorite(item.itemId)}
        />
      ))}
    </ul>
  );
}

interface ResultRowProps {
  item: Item;
  highlighted: boolean;
  favorite: boolean;
  onHighlight: () => void;
  onSelect: () => void;
  onToggleFavorite: () => void;
}

function ResultRow({
  item,
  highlighted,
  favorite,
  onHighlight,
  onSelect,
  onToggleFavorite,
}: ResultRowProps) {
  const ref = useRef<HTMLLIElement>(null);

  // Keyboard navigation has to keep the highlighted row on screen.
  useEffect(() => {
    if (highlighted) ref.current?.scrollIntoView({ block: "nearest" });
  }, [highlighted]);

  const icon = iconUrl(item.iconPath);

  return (
    <li
      ref={ref}
      className={`result${highlighted ? " is-highlighted" : ""}`}
      role="option"
      aria-selected={highlighted}
      onMouseEnter={onHighlight}
      onClick={onSelect}
    >
      {icon ? (
        <img className="result-icon" src={icon} alt="" loading="lazy" />
      ) : (
        <span className="result-icon result-icon-empty" />
      )}
      <span className="result-name">{item.name}</span>
      {item.categoryName && (
        <span className="result-category">{item.categoryName}</span>
      )}
      {/* A hearted row always shows its heart; the rest only reveal one under
          the cursor or the keyboard highlight, so a list of search results is
          not a wall of buttons. `stopPropagation` keeps the click off the row,
          which would otherwise open the item. */}
      <button
        type="button"
        className={`result-heart${favorite ? " is-favorite" : ""}`}
        onClick={(event) => {
          event.stopPropagation();
          onToggleFavorite();
        }}
        title={favorite ? "Remove from favorites" : "Add to favorites"}
        aria-label={favorite ? "Remove from favorites" : "Add to favorites"}
        aria-pressed={favorite}
      >
        <HeartIcon size={12} filled={favorite} />
      </button>
    </li>
  );
}

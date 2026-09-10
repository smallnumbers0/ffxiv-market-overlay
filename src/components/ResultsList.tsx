import { useEffect, useRef } from "react";

import type { Item, SearchResult } from "../lib/tauriApi";
import { iconUrl } from "../lib/tauriApi";

interface ResultsListProps {
  results: SearchResult[] | Item[];
  highlightIndex: number;
  onHighlight: (index: number) => void;
  onSelect: (item: Item) => void;
  emptyMessage?: string;
}

export function ResultsList({
  results,
  highlightIndex,
  onHighlight,
  onSelect,
  emptyMessage,
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
          onHighlight={() => onHighlight(index)}
          onSelect={() => onSelect(item)}
        />
      ))}
    </ul>
  );
}

interface ResultRowProps {
  item: Item;
  highlighted: boolean;
  onHighlight: () => void;
  onSelect: () => void;
}

function ResultRow({ item, highlighted, onHighlight, onSelect }: ResultRowProps) {
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
    </li>
  );
}

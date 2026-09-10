import { forwardRef } from "react";

import { SearchIcon } from "./icons";

interface SearchBoxProps {
  value: string;
  onChange: (value: string) => void;
  onKeyDown: (event: React.KeyboardEvent<HTMLInputElement>) => void;
  disabled?: boolean;
  placeholder?: string;
}

export const SearchBox = forwardRef<HTMLInputElement, SearchBoxProps>(
  function SearchBox(
    { value, onChange, onKeyDown, disabled, placeholder = "Search items..." },
    ref,
  ) {
    return (
      <div className="searchbox">
        <span className="searchbox-icon">
          <SearchIcon />
        </span>
        <input
          ref={ref}
          className="searchbox-input"
          type="text"
          value={value}
          disabled={disabled}
          placeholder={placeholder}
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={onKeyDown}
          // The overlay is opened by a hotkey to type into, so never make the
          // user click the field first.
          autoFocus
          autoComplete="off"
          spellCheck={false}
          aria-label="Search items"
        />
        {value && (
          <button
            type="button"
            className="icon-button searchbox-clear"
            onClick={() => onChange("")}
            title="Clear"
            aria-label="Clear search"
          >
            &times;
          </button>
        )}
      </div>
    );
  },
);

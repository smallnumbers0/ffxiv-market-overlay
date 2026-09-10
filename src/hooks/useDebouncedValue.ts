import { useEffect, useState } from "react";

/**
 * Returns `value` after it has stopped changing for `delayMs`.
 *
 * Search runs locally in Rust, so this only exists to coalesce a fast typist's
 * keystrokes into one scoring pass - not to spare a network call. That's why
 * the delay is far below the ~150ms a network-backed search would want.
 */
export function useDebouncedValue<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);

  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), delayMs);
    return () => clearTimeout(timer);
  }, [value, delayMs]);

  return debounced;
}

/**
 * Icons, drawn on a 16x16 grid with 1.5px strokes on half-pixel coordinates so
 * they stay crisp at the 14px they actually render at. Detailed shapes (a
 * toothed gear, say) turn to mush at this size - each of these is built from
 * two or three strokes that survive it.
 */

type IconProps = { size?: number };

function Svg({ size = 14, children }: IconProps & { children: React.ReactNode }) {
  return (
    <svg
      viewBox="0 0 16 16"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

/** Sliders. Reads as "settings" far more clearly than a gear does at 14px. */
export const SettingsIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M2.5 4.5h11M2.5 8.5h11M2.5 12.5h11" />
    <circle cx="5.5" cy="4.5" r="1.6" fill="currentColor" stroke="none" />
    <circle cx="10.5" cy="8.5" r="1.6" fill="currentColor" stroke="none" />
    <circle cx="6.5" cy="12.5" r="1.6" fill="currentColor" stroke="none" />
  </Svg>
);

export const CloseIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="m4.5 4.5 7 7m0-7-7 7" />
  </Svg>
);

/** Plus. Adds a market board tab. */
export const PlusIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M8 3.5v9M3.5 8h9" />
  </Svg>
);

export const SearchIcon = (props: IconProps) => (
  <Svg {...props}>
    <circle cx="7" cy="7" r="4.4" />
    <path d="m10.4 10.4 3 3" />
  </Svg>
);

/** Circular arrow. The gap plus arrowhead is what makes it read as "refresh". */
export const RefreshIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M13 8a5 5 0 1 1-1.6-3.7" />
    <path d="M13.2 2.3v2.9h-2.9" />
  </Svg>
);

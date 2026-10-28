// Icons of the welcome window, drawn after TablePlus' (SF Symbols-like) originals.

const STROKE = {
  fill: "none",
  stroke: "currentColor",
  strokeLinecap: "round",
  strokeLinejoin: "round",
} as const;

/** A group: a plug whose cord runs into a ring. */
export function PlugCircleIcon({ size = 27 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 27 27" aria-hidden className="plug-icon">
      <g {...STROKE} strokeWidth={2.2}>
        <path d="M8.6 24.8A12.4 12.4 0 1 1 16.6 25.5" />
        <path d="M13.5 19v3.6q0 3.1 3.1 2.9" />
      </g>
      <g fill="currentColor">
        <path d="M9.6 11.2h7.8V14c0 2.9-1.7 5.2-3.9 5.2S9.6 16.9 9.6 14z" />
        <rect x="11" y="6.6" width="1.6" height="5" rx="0.8" />
        <rect x="14.4" y="6.6" width="1.6" height="5" rx="0.8" />
      </g>
    </svg>
  );
}

export function PlusIcon({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 14 14" aria-hidden>
      <path d="M7 0.8v12.4M0.8 7h12.4" {...STROKE} strokeWidth={1.6} />
    </svg>
  );
}

const FOLDER =
  "M14.4 3.4H9L7.2 1.4H3.6a1.8 1.8 0 0 0-1.8 1.8v10.2a1.8 1.8 0 0 0 1.8 1.8h15.1" +
  "a1.8 1.8 0 0 0 1.8-1.8V9.6";

/** A folder with a "+" badge (new group). */
export function FolderPlusIcon({ width = 24 }: { width?: number }) {
  return (
    <svg width={width} height={(width * 17) / 24} viewBox="0 0 24 17" aria-hidden>
      <g {...STROKE} strokeWidth={1.6}>
        <path d={FOLDER} />
        <path d="M1.8 5.9h12.4" />
      </g>
      <circle cx="19.2" cy="4.6" r="4.6" fill="currentColor" />
      <path d="M19.2 2.4v4.4M17 4.6h4.4" {...STROKE} strokeWidth={1.5} className="badge-mark" />
    </svg>
  );
}

/** An inbox tray with an arrow leaving it (backup) or entering it (restore). */
export function TrayArrowIcon(props: { direction: "up" | "down"; size?: number }) {
  const { direction, size = 17 } = props;
  const arrow =
    direction === "up" ? "M12 15V2.5M8.5 6 12 2.5 15.5 6" : "M12 2.5V14M8.5 10.5 12 14l3.5-3.5";
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden>
      <g {...STROKE} strokeWidth={2}>
        <path d="M3 13.5V19a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-5.5" />
        <path d="M3 13.5 6 8.5h2.5M21 13.5l-3-5h-2.5" />
        <path d="M3 13.5h5.5a3.5 3.5 0 0 0 7 0H21" />
        <path d={arrow} />
      </g>
    </svg>
  );
}

/** The disclosure chevron of a group (points right when collapsed). */
export function ChevronIcon({ open }: { open: boolean }) {
  return (
    <svg
      width="9"
      height="6"
      viewBox="0 0 10 6"
      aria-hidden
      className={`tree-chevron${open ? "" : " closed"}`}
    >
      <path d="M1 1l4 4 4-4" {...STROKE} strokeWidth={1.6} />
    </svg>
  );
}

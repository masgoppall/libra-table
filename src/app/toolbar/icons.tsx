// The workspace toolbar's glyphs, drawn at the sizes TablePlus' toolbar uses (in points).

const line = {
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.3,
  strokeLinecap: "round",
  strokeLinejoin: "round",
} as const;

function Glyph(props: { w: number; h: number; children: React.ReactNode }) {
  return (
    <svg width={props.w} height={props.h} viewBox={`0 0 ${props.w} ${props.h}`} aria-hidden>
      {props.children}
    </svg>
  );
}

export const SidebarIcon = () => (
  <Glyph w={17} h={13}>
    <rect x="0.65" y="0.65" width="15.7" height="11.7" rx="2.6" {...line} />
    <path d="M6 0.9v11.2M2.6 3.6h1.6M2.6 5.8h1.6M2.6 8h1.6" {...line} />
  </Glyph>
);

export const PlugIcon = () => (
  <Glyph w={14} h={14}>
    <circle cx="7" cy="7" r="6.35" {...line} />
    <path d="M5.4 3.4v2M8.6 3.4v2M7 9.6v1.6" {...line} />
    <path d="M4.3 5.4h5.4v1.4a2.7 2.7 0 0 1-5.4 0z" fill="currentColor" stroke="none" />
  </Glyph>
);

export const DiscardIcon = () => (
  <Glyph w={14} h={14}>
    <circle cx="7" cy="7" r="6.35" {...line} />
    <path d="M4.7 4.7l4.6 4.6M9.3 4.7l-4.6 4.6" {...line} />
  </Glyph>
);

export const PreviewIcon = () => (
  <Glyph w={20} h={13}>
    <path
      d="M1 6.5C3.2 2.7 6.4 0.8 10 0.8s6.8 1.9 9 5.7c-2.2 3.8-5.4 5.7-9 5.7S3.2 10.3 1 6.5z"
      {...line}
    />
    <circle cx="10" cy="6.5" r="2.7" fill="currentColor" />
  </Glyph>
);

export const CommitIcon = () => (
  <Glyph w={17} h={13}>
    <path d="M1 1.6h9M1 4.8h9M1 8h9M1 11.2h9M14 11.6V1.4M11.6 3.8 14 1.4l2.4 2.4" {...line} />
  </Glyph>
);

export const LockIcon = () => (
  <Glyph w={15} h={14}>
    <rect x="0.65" y="6" width="9.2" height="7.35" rx="1.8" {...line} />
    <path d="M7.4 6V3.9a3.2 3.2 0 0 1 6.4 0v1.5" {...line} />
  </Glyph>
);

export const DatabaseIcon = () => (
  <Glyph w={15} h={15}>
    <ellipse cx="7.5" cy="3" rx="6.4" ry="2.3" {...line} />
    <path
      d="M1.1 3v9c0 1.3 2.9 2.3 6.4 2.3s6.4-1 6.4-2.3V3M1.1 7.5c0 1.3 2.9 2.3 6.4 2.3s6.4-1 6.4-2.3"
      {...line}
    />
  </Glyph>
);

export const RefreshIcon = () => (
  <Glyph w={13} h={15}>
    <path d="M6.5 3.2a5.5 5.5 0 1 0 5.5 5.5" {...line} />
    <path d="M5 0.9l2.4 2.3L5 5.5" {...line} />
  </Glyph>
);

export const MetricsIcon = () => (
  <Glyph w={18} h={13}>
    <rect x="0.65" y="6" width="4" height="6.35" rx="1.2" {...line} />
    <rect x="6.8" y="3.4" width="4" height="8.95" rx="1.2" {...line} />
    <rect x="13" y="0.65" width="4" height="11.7" rx="1.2" {...line} />
  </Glyph>
);

export const CommandIcon = () => (
  <Glyph w={13} h={13}>
    <path d="M4.3 4.3h4.4v4.4H4.3z" {...line} />
    <path d="M4.3 4.3V2.6a1.7 1.7 0 1 0-1.7 1.7zM8.7 4.3V2.6a1.7 1.7 0 1 1 1.7 1.7z" {...line} />
    <path d="M4.3 8.7v1.7a1.7 1.7 0 1 1-1.7-1.7zM8.7 8.7v1.7a1.7 1.7 0 1 0 1.7-1.7z" {...line} />
  </Glyph>
);

export const ConsoleIcon = () => (
  <Glyph w={17} h={13}>
    <rect x="0.65" y="0.65" width="15.7" height="11.7" rx="2.6" {...line} />
    <path d="M0.9 7.4h15.2" {...line} />
    <path d="M4.6 9.9h.1M8.5 9.9h.1M12.4 9.9h.1" {...line} strokeWidth={1.6} />
  </Glyph>
);

export const DetailIcon = () => (
  <Glyph w={17} h={13}>
    <rect x="0.65" y="0.65" width="15.7" height="11.7" rx="2.6" {...line} />
    <path d="M11 0.9v11.2M12.9 3.6h1.6M12.9 5.8h1.6M12.9 8h1.6" {...line} />
  </Glyph>
);

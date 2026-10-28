// A macOS-style sheet: a fixed-size panel without a title bar, its buttons at the bottom
// (the driver picker, "Open database"). Escape and ⌘W close it, like a modal.

import { type ReactNode } from "react";
import { useDialogLayer } from "./Modal";

export function Sheet(props: {
  label: string;
  onClose: () => void;
  children: ReactNode;
  footer: ReactNode;
  className?: string;
}) {
  useDialogLayer(props.onClose);
  return (
    <div
      className="sheet-overlay"
      onMouseDown={(e) => e.target === e.currentTarget && props.onClose()}
    >
      <div
        className={`sheet ${props.className ?? ""}`.trim()}
        role="dialog"
        aria-modal="true"
        aria-label={props.label}
      >
        {props.children}
        <div className="sheet-foot">{props.footer}</div>
      </div>
    </div>
  );
}

/** The rounded search field at the top of a sheet. */
export function SheetSearch(props: {
  value: string;
  onChange: (v: string) => void;
  placeholder: string;
  onKeyDown?: (e: React.KeyboardEvent) => void;
}) {
  return (
    <label className="sheet-search">
      <svg width="13" height="13" viewBox="0 0 15 15" aria-hidden className="sheet-search-icon">
        <circle cx="6.2" cy="6.2" r="5" fill="none" stroke="currentColor" strokeWidth="1.4" />
        <path d="m9.9 9.9 4 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      </svg>
      <input
        autoFocus
        value={props.value}
        placeholder={props.placeholder}
        onChange={(e) => props.onChange(e.target.value)}
        onKeyDown={props.onKeyDown}
      />
    </label>
  );
}

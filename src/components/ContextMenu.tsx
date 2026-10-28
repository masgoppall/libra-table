// Context menus at the pointer (or under a button), kept inside the window.

import { type ReactNode, useEffect, useLayoutEffect, useRef, useState } from "react";

export interface MenuItem {
  label: string;
  onClick?: () => void;
  danger?: boolean;
  disabled?: boolean;
  separator?: boolean;
  icon?: ReactNode;
  hint?: string;
  /** The current value of a popup menu: highlighted when it opens. */
  selected?: boolean;
  /** A check mark in front (items that toggle). */
  checked?: boolean;
}

export const SEPARATOR: MenuItem = { label: "", separator: true };

const EDGE = 8;
/** `.menu-item` height and `.menu` padding, to put a popup's current item over its cell. */
const ITEM_HEIGHT = 24;
const MENU_PADDING = 5;

interface MenuProps {
  x: number;
  y: number;
  items: MenuItem[];
  onClose: () => void;
  /** Wide enough to cover the control it opened from (popup menus). */
  minWidth?: number;
  /** Opens upwards: the menu's bottom edge sits here (buttons at the window bottom). */
  bottom?: number;
}

/** Closes on any outside click, blur, resize or Escape. */
function useDismiss(onClose: () => void) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    const events = ["mousedown", "blur", "resize"] as const;
    events.forEach((ev) => window.addEventListener(ev, onClose));
    window.addEventListener("keydown", onKey);
    return () => {
      events.forEach((ev) => window.removeEventListener(ev, onClose));
      window.removeEventListener("keydown", onKey);
    };
  }, [onClose]);
}

function MenuRow({
  item,
  hasIcons,
  close,
}: {
  item: MenuItem;
  hasIcons: boolean;
  close: () => void;
}) {
  if (item.separator) return <div className="menu-sep" />;
  const classes = [
    "menu-item",
    item.danger && "danger",
    item.disabled && "disabled",
    item.selected && "selected",
  ];
  return (
    <div
      role="menuitem"
      className={classes.filter(Boolean).join(" ")}
      onClick={() => {
        if (item.disabled) return;
        close();
        item.onClick?.();
      }}
    >
      {hasIcons && <span className="menu-icon">{item.icon}</span>}
      {item.checked !== undefined && (
        <span className="menu-check" aria-hidden>
          {item.checked ? "✓" : ""}
        </span>
      )}
      <span className="menu-label">{item.label}</span>
      {item.hint && <span className="menu-hint">{item.hint}</span>}
    </div>
  );
}

export function ContextMenu(props: MenuProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x: props.x, y: props.y });
  useLayoutEffect(() => {
    const r = ref.current?.getBoundingClientRect();
    if (!r) return;
    const x = Math.max(4, Math.min(props.x, window.innerWidth - r.width - EDGE));
    const top = props.bottom === undefined ? props.y : props.bottom - r.height;
    const y = Math.max(4, Math.min(top, window.innerHeight - r.height - EDGE));
    setPos({ x, y });
    ref.current?.querySelector(".menu-item.selected")?.scrollIntoView?.({ block: "nearest" });
  }, [props.x, props.y, props.bottom]);
  useDismiss(props.onClose);
  const hasIcons = props.items.some((i) => i.icon);
  return (
    <div
      className="menu"
      ref={ref}
      style={{ left: pos.x, top: pos.y, minWidth: props.minWidth }}
      onMouseDown={(e) => e.stopPropagation()}
      role="menu"
    >
      {props.items.map((item, i) => (
        <MenuRow key={i} item={item} hasIcons={hasIcons} close={props.onClose} />
      ))}
    </div>
  );
}

interface PointerLike {
  clientX: number;
  clientY: number;
  preventDefault?: () => void;
  stopPropagation?: () => void;
}

/** `open` at the pointer, `openAt` under an element; render `node` somewhere. */
export function useContextMenu() {
  const [menu, setMenu] = useState<Omit<MenuProps, "onClose"> | null>(null);
  const open = (e: PointerLike, items: MenuItem[]) => {
    e.preventDefault?.();
    e.stopPropagation?.();
    setMenu({ x: e.clientX, y: e.clientY, items });
  };
  const openAt = (el: HTMLElement, items: MenuItem[]) => {
    const r = el.getBoundingClientRect();
    setMenu({ x: r.left, y: r.bottom + 4, items });
  };
  /** A popup menu over `el`, its selected item where the control's value is. */
  const openOver = (el: HTMLElement, items: MenuItem[]) => {
    const r = el.getBoundingClientRect();
    const rows = items.slice(
      0,
      Math.max(
        0,
        items.findIndex((i) => i.selected),
      ),
    );
    const above = rows.reduce((h, i) => h + (i.separator ? 11 : ITEM_HEIGHT), 0);
    const y = r.top + (r.height - ITEM_HEIGHT) / 2 - MENU_PADDING - above;
    setMenu({ x: r.left - MENU_PADDING, y, items, minWidth: r.width + 2 * MENU_PADDING });
  };
  const openAbove = (el: HTMLElement, items: MenuItem[]) => {
    const r = el.getBoundingClientRect();
    setMenu({ x: r.left, y: r.top, bottom: r.top - 4, items });
  };
  const close = () => setMenu(null);
  const node = menu ? <ContextMenu {...menu} onClose={close} /> : null;
  return { open, openAt, openOver, openAbove, node };
}

export type ContextMenuApi = ReturnType<typeof useContextMenu>;

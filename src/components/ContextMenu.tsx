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
}

export const SEPARATOR: MenuItem = { label: "", separator: true };

const EDGE = 8;

interface MenuProps {
  x: number;
  y: number;
  items: MenuItem[];
  onClose: () => void;
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
  const classes = ["menu-item", item.danger && "danger", item.disabled && "disabled"];
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
    const y = Math.max(4, Math.min(props.y, window.innerHeight - r.height - EDGE));
    setPos({ x, y });
  }, [props.x, props.y]);
  useDismiss(props.onClose);
  const hasIcons = props.items.some((i) => i.icon);
  return (
    <div
      className="menu"
      ref={ref}
      style={{ left: pos.x, top: pos.y }}
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
  const close = () => setMenu(null);
  const node = menu ? <ContextMenu {...menu} onClose={close} /> : null;
  return { open, openAt, node };
}

export type ContextMenuApi = ReturnType<typeof useContextMenu>;

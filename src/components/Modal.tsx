// Dialog frame. While open, app shortcuts don't act on what is behind it.

import { X } from "lucide-react";
import { type ReactNode, useEffect, useRef } from "react";
import { LAYER, useActions } from "@/lib/actions";

export type ModalSize = "sm" | "md" | "lg" | "xl";

export interface ModalProps {
  title: ReactNode;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  size?: ModalSize;
  icon?: ReactNode;
  subtitle?: ReactNode;
  className?: string;
}

/** Closes on Escape before anything else sees it. */
function useEscape(onClose: () => void) {
  const ref = useRef(onClose);
  ref.current = onClose;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      ref.current();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);
  return ref;
}

export function Modal(props: ModalProps) {
  const onClose = useEscape(props.onClose);
  const swallow = () => true;
  useActions(LAYER.dialog, true, {
    "close-tab": () => onClose.current(),
    "new-query": swallow,
    "next-tab": swallow,
    "prev-tab": swallow,
    "open-anything": swallow,
    "switch-database": swallow,
  });
  const classes = ["modal", props.size ?? "md", props.className].filter(Boolean).join(" ");
  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && props.onClose()}>
      <div className={classes} role="dialog" aria-modal="true">
        <div className="modal-head">
          {props.icon && <span className="modal-icon">{props.icon}</span>}
          <div className="modal-titles">
            <div className="modal-title">{props.title}</div>
            {props.subtitle && <div className="modal-subtitle">{props.subtitle}</div>}
          </div>
          <button className="btn ghost icon sm" onClick={props.onClose} aria-label="Close">
            <X size={15} />
          </button>
        </div>
        <div className="modal-body">{props.children}</div>
        {props.footer && <div className="modal-foot">{props.footer}</div>}
      </div>
    </div>
  );
}

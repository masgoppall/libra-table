// Loading and empty states, centered in whatever area they fill.

import { type ReactNode, useLayoutEffect, useRef, useState } from "react";
import { cx } from "@/lib/cx";
import { type ArtName } from "@/types";
import { ART_HEIGHT, ART_WIDTH, BrandArt } from "./BrandArt";
import { Spinner } from "./Spinner";

/** A spinner in the middle of the area, with an optional line under it. */
export function LoadingState({ label, compact }: { label?: string; compact?: boolean }) {
  return (
    <div className={cx("state-view", compact && "compact")} role="status" aria-live="polite">
      <Spinner size={compact ? 16 : 20} className="state-spinner" />
      {label && <div className="state-label">{label}</div>}
    </div>
  );
}

/** Room the illustration needs around itself, the title, the hint and an action. */
const ART_ROOM = { width: ART_WIDTH + 48, height: ART_HEIGHT + 170 };

/** Whether the area holding the state (its parent) has room for the illustration. */
function useRoomForArt(enabled: boolean) {
  const ref = useRef<HTMLDivElement>(null);
  const [fits, setFits] = useState(false);
  useLayoutEffect(() => {
    const area = ref.current?.parentElement;
    if (!enabled || !area || typeof ResizeObserver === "undefined") return;
    const read = () =>
      setFits(area.clientWidth >= ART_ROOM.width && area.clientHeight >= ART_ROOM.height);
    read();
    const ro = new ResizeObserver(read);
    ro.observe(area);
    return () => ro.disconnect();
  }, [enabled]);
  return { ref, fits };
}

/** Nothing to show yet: an illustration (or an icon where it doesn't fit), what is (not)
 * here and what to do about it. */
export function EmptyState(props: {
  icon?: ReactNode;
  /** One of the brand illustrations, shown when the area is large enough. */
  art?: ArtName;
  title: ReactNode;
  hint?: ReactNode;
  action?: ReactNode;
  compact?: boolean;
}) {
  const room = useRoomForArt(!!props.art);
  const icon = props.icon && <div className="state-icon">{props.icon}</div>;
  return (
    <div ref={room.ref} className={cx("state-view", props.compact && "compact")}>
      {props.art && room.fits ? (
        <BrandArt name={props.art} className="state-art" fallback={icon} />
      ) : (
        icon
      )}
      <div className="state-title">{props.title}</div>
      {props.hint && <div className="state-hint">{props.hint}</div>}
      {props.action && <div className="state-action">{props.action}</div>}
    </div>
  );
}

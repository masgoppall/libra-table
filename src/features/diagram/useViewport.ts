// Pan and zoom of the diagram: drag the background, wheel or pinch to zoom, buttons, fit.

import { useEffect, useRef, useState } from "react";
import { startDrag } from "@/lib/drag";
import { fitView, type Rect, type View, zoomAt } from "./layout";

const WHEEL_STEP = 0.0015;

/** `ready`: the canvas element is on screen (it appears once the tables load). */
export function useViewport(ready: boolean) {
  const ref = useRef<HTMLDivElement>(null);
  const [view, setView] = useState<View>({ x: 32, y: 32, scale: 1 });

  // A native listener: React's wheel listener is passive and can't stop page zoom.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const box = el.getBoundingClientRect();
      const at = { x: e.clientX - box.left, y: e.clientY - box.top };
      // Pinch arrives as ctrl+wheel with small deltas: zoom faster then.
      const step = e.ctrlKey ? WHEEL_STEP * 6 : WHEEL_STEP;
      setView((v) => zoomAt(v, Math.exp(-e.deltaY * step), at));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [ready]);

  const size = () => ({ w: ref.current?.clientWidth ?? 800, h: ref.current?.clientHeight ?? 600 });

  const zoomBy = (factor: number) => {
    const s = size();
    setView((v) => zoomAt(v, factor, { x: s.w / 2, y: s.h / 2 }));
  };

  const fit = (content: Rect) => setView(fitView(content, size()));

  const startPan = (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    const from = view;
    startDrag(e, {
      bodyClass: "dragging-grab",
      onMove: (dx, dy) => setView({ ...from, x: from.x + dx, y: from.y + dy }),
    });
  };

  return { ref, view, zoomBy, fit, startPan, reset: () => setView({ x: 32, y: 32, scale: 1 }) };
}

export type Viewport = ReturnType<typeof useViewport>;

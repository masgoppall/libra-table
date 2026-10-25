import { useState } from "react";
import { clamp, startDrag } from "@/lib/drag";
import { readNumber, writeValue } from "@/lib/localStore";

const KEY = "libratable.sidebarWidth";
export const SIDEBAR_WIDTH = { min: 200, max: 520, initial: 264 };

/** The sidebar width, dragged by its edge and remembered per viewer. */
export function useSidebarWidth() {
  const { min, max, initial } = SIDEBAR_WIDTH;
  const [width, setWidth] = useState(() => readNumber(KEY, initial, min, max));

  const startResize = (e: React.MouseEvent) => {
    const start = width;
    let last = start;
    startDrag(e, {
      bodyClass: "resizing",
      onMove: (dx) => {
        last = clamp(start + dx, min, max);
        setWidth(last);
      },
      onEnd: () => writeValue(KEY, last),
    });
  };

  const reset = () => {
    setWidth(initial);
    writeValue(KEY, initial);
  };

  return { width, startResize, reset };
}

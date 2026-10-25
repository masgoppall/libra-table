import { useState } from "react";
import { clamp, startDrag } from "@/lib/drag";

const MIN = 12;
const MAX = 85;

/** Editor height in % of the tab, dragged by the splitter. */
export function useEditorSplit(initial = 42) {
  const [height, setHeight] = useState(initial);
  const onSplitterDown = (e: React.MouseEvent) => {
    const box = (e.currentTarget.parentElement as HTMLElement).getBoundingClientRect();
    startDrag(e, {
      bodyClass: "resizing-row",
      onMove: (_dx, _dy, ev) =>
        setHeight(clamp(((ev.clientY - box.top) / box.height) * 100, MIN, MAX)),
    });
  };
  return { height, onSplitterDown };
}

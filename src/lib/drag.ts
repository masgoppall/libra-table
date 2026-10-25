// Mouse drags for splitters and resize handles: follow the pointer until release.

export interface DragOptions {
  /** Called with the distance from where the drag started. */
  onMove: (dx: number, dy: number, e: MouseEvent) => void;
  onEnd?: () => void;
  /** Class on <body> while dragging (keeps the cursor, blocks text selection). */
  bodyClass?: string;
}

export function startDrag(e: React.MouseEvent, opts: DragOptions) {
  e.preventDefault();
  const startX = e.clientX;
  const startY = e.clientY;
  const move = (ev: MouseEvent) => opts.onMove(ev.clientX - startX, ev.clientY - startY, ev);
  const up = () => {
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mouseup", up);
    if (opts.bodyClass) document.body.classList.remove(opts.bodyClass);
    opts.onEnd?.();
  };
  if (opts.bodyClass) document.body.classList.add(opts.bodyClass);
  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", up);
}

export function clamp(n: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, n));
}

// Exporting the diagram: a standalone SVG of the whole diagram (theme colors inlined,
// since CSS variables don't travel), or a PNG drawn from it at twice the size.

import { type Rect } from "./layout";

const PAD = 24;
const STYLE_PROPS = [
  "fill",
  "stroke",
  "stroke-width",
  "stroke-dasharray",
  "opacity",
  "font-family",
  "font-size",
  "font-weight",
] as const;

function inlineStyles(live: Element, copy: Element) {
  const from = [live, ...Array.from(live.querySelectorAll("*"))];
  const to = [copy, ...Array.from(copy.querySelectorAll("*"))];
  from.forEach((el, i) => {
    const target = to[i];
    if (!target) return;
    const cs = window.getComputedStyle(el);
    const style = STYLE_PROPS.map((p) => `${p}:${cs.getPropertyValue(p)}`)
      .filter((s) => !s.endsWith(":"))
      .join(";");
    target.setAttribute("style", style);
    target.removeAttribute("class");
  });
}

/** The diagram's SVG text, framed around `content` (diagram coordinates). */
export function diagramSvg(svg: SVGSVGElement, content: Rect, background: string): string {
  const copy = svg.cloneNode(true) as SVGSVGElement;
  inlineStyles(svg, copy);
  const g = copy.querySelector("[data-diagram-content]");
  g?.setAttribute("transform", "");
  const [x, y] = [content.x - PAD, content.y - PAD];
  const [w, h] = [content.w + PAD * 2, content.h + PAD * 2];
  copy.setAttribute("xmlns", "http://www.w3.org/2000/svg");
  copy.setAttribute("viewBox", `${x} ${y} ${w} ${h}`);
  copy.setAttribute("width", String(Math.ceil(w)));
  copy.setAttribute("height", String(Math.ceil(h)));
  const bg = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  for (const [k, v] of Object.entries({ x, y, width: w, height: h, fill: background })) {
    bg.setAttribute(k, String(v));
  }
  copy.insertBefore(bg, copy.firstChild);
  return new XMLSerializer().serializeToString(copy);
}

/** A PNG (base64, no data: prefix) of an SVG text, `scale` times its size. */
export function svgToPng(svgText: string, w: number, h: number, scale = 2): Promise<string> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    const url = URL.createObjectURL(new Blob([svgText], { type: "image/svg+xml" }));
    img.onload = () => {
      const canvas = document.createElement("canvas");
      canvas.width = Math.ceil(w * scale);
      canvas.height = Math.ceil(h * scale);
      const ctx = canvas.getContext("2d");
      URL.revokeObjectURL(url);
      if (!ctx) return reject(new Error("Canvas is not available."));
      ctx.scale(scale, scale);
      ctx.drawImage(img, 0, 0, w, h);
      resolve(canvas.toDataURL("image/png").replace(/^data:image\/png;base64,/, ""));
    };
    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error("The diagram could not be drawn as an image."));
    };
    img.src = url;
  });
}

export const exportSize = (content: Rect) => ({ w: content.w + PAD * 2, h: content.h + PAD * 2 });

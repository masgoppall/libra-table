// The empty-state illustrations of public/brand, drawn in the app's theme. Each SVG keeps
// its dark colors in a prefers-color-scheme rule, which inside an <img> follows the system;
// the rule is kept (dark) or dropped (light) here so a theme set in Settings wins. The SVGs
// load in their own chunk right after startup; until then the caller's icon shows.

import { type ReactNode, useEffect, useState } from "react";
import { useDarkMode } from "@/hooks/useDarkMode";
import { cx } from "@/lib/cx";
import { useSettings } from "@/store";
import { type ArtName } from "@/types";

export const ART_WIDTH = 240;
export const ART_HEIGHT = 170;

const DARK_RULE = /@media \(prefers-color-scheme:dark\)\{(svg\.lt\{[^}]*\})\}/;

let art: Record<string, string> | null = null;
/** Resolves once the illustrations are loaded. */
export const artLoaded: Promise<void> = import("virtual:brand-art").then((m) => {
  art = m.default;
});
const urls = new Map<string, string>();

/** The illustration as a data URL in light or dark colors; null until loaded or unknown. */
export function artUrl(name: ArtName, dark: boolean): string | null {
  const key = `${name}:${dark}`;
  const cached = urls.get(key);
  if (cached) return cached;
  const svg = art?.[name];
  if (!svg) return null;
  const themed = svg.replace(DARK_RULE, dark ? "$1" : "");
  const url = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(themed)}`;
  urls.set(key, url);
  return url;
}

function useArtUrl(name: ArtName): string | null {
  const settings = useSettings();
  const dark = useDarkMode(settings?.theme);
  const [, setLoaded] = useState(art !== null);
  useEffect(() => {
    if (art) return;
    let live = true;
    void artLoaded.then(() => live && setLoaded(true));
    return () => {
      live = false;
    };
  }, []);
  return artUrl(name, dark);
}

/** The illustration, or `fallback` while it loads. */
export function BrandArt(props: { name: ArtName; className?: string; fallback?: ReactNode }) {
  const src = useArtUrl(props.name);
  if (!src) return <>{props.fallback ?? null}</>;
  return (
    <img
      className={cx("brand-art", props.className)}
      src={src}
      width={ART_WIDTH}
      height={ART_HEIGHT}
      alt=""
      draggable={false}
    />
  );
}

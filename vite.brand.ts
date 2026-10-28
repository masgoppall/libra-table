// Brand artwork lives in public/brand (logo, driver badges, empty states, design notes).
// Vite copies public/ into the app as is, so this keeps what the app needs and no more:
// the empty-state SVGs become a module (the UI recolors them for its theme), and the
// build drops every brand file except the driver badges the UI loads by URL.

import { readdirSync, readFileSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { type Plugin } from "vite";

const ART_MODULE = "virtual:brand-art";
const RESOLVED_ART = `\0${ART_MODULE}`;
/** Folders of public/brand the built app loads by URL. */
const SHIPPED = ["drivers/badge"];

/** `import art from "virtual:brand-art"`: { "no-rows": "<svg …>", … }. */
export function brandArt(publicDir: string): Plugin {
  const dir = join(publicDir, "brand", "empty-states");
  return {
    name: "libratable:brand-art",
    resolveId: (id) => (id === ART_MODULE ? RESOLVED_ART : undefined),
    load(id) {
      if (id !== RESOLVED_ART) return undefined;
      const art: Record<string, string> = {};
      for (const file of readdirSync(dir).filter((f) => f.endsWith(".svg"))) {
        const path = join(dir, file);
        this.addWatchFile(path);
        art[file.slice(0, -".svg".length)] = readFileSync(path, "utf8");
      }
      return `export default ${JSON.stringify(art)};`;
    },
  };
}

const shipped = (rel: string) => SHIPPED.some((s) => rel === s || rel.startsWith(`${s}/`));
const holdsShipped = (rel: string) => SHIPPED.some((s) => s.startsWith(`${rel}/`));

function prune(dir: string, rel: string) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    const sub = rel ? `${rel}/${entry.name}` : entry.name;
    if (shipped(sub)) continue;
    if (entry.isDirectory() && holdsShipped(sub)) prune(path, sub);
    else rmSync(path, { recursive: true, force: true });
  }
}

/** After a build: removes dist/brand's design files, keeping only SHIPPED. */
export function pruneBrand(): Plugin {
  let outDir = "";
  return {
    name: "libratable:prune-brand",
    apply: "build",
    configResolved(config) {
      outDir = resolve(config.root, config.build.outDir);
    },
    closeBundle() {
      const brand = join(outDir, "brand");
      try {
        prune(brand, "");
      } catch (e) {
        if ((e as NodeJS.ErrnoException).code !== "ENOENT") throw e;
      }
    },
  };
}

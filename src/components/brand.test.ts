// The brand assets the UI loads: a badge for every driver, every illustration, and the
// illustrations' colors following the app's theme rather than the system's.

import { existsSync } from "node:fs";
import { describe, expect, it } from "vitest";
import art from "virtual:brand-art";
import { type ArtName, type Driver } from "@/types";
import { artLoaded, artUrl } from "./BrandArt";

// Records so that a new driver or illustration name fails to compile until listed here.
const DRIVERS: Record<Driver, true> = {
  postgres: true,
  mysql: true,
  sqlite: true,
  mariadb: true,
  cockroach: true,
  redshift: true,
  greenplum: true,
  vertica: true,
  libsql: true,
  cloudflare_d1: true,
  mssql: true,
  oracle: true,
  clickhouse: true,
  duckdb: true,
  cassandra: true,
  snowflake: true,
  bigquery: true,
  redis: true,
  mongodb: true,
  dynamodb: true,
  elasticsearch: true,
};
const ARTS: Record<ArtName, true> = {
  board: true,
  "connection-failed": true,
  console: true,
  "key-missing": true,
  "no-connections": true,
  "no-history": true,
  "no-results": true,
  "no-rows": true,
  "no-tables": true,
  "query-results": true,
};

const svgOf = (url: string | null) =>
  decodeURIComponent((url ?? "").replace("data:image/svg+xml;charset=utf-8,", ""));

describe("brand assets", () => {
  it("has a badge for every driver", () => {
    const missing = Object.keys(DRIVERS).filter(
      (d) => !existsSync(`public/brand/drivers/badge/${d}.svg`),
    );
    expect(missing).toEqual([]);
  });

  it("has every illustration the UI names", () => {
    expect(Object.keys(art).sort()).toEqual(Object.keys(ARTS).sort());
  });

  it("draws the illustrations in the app's theme, not the system's", async () => {
    await artLoaded;
    for (const name of Object.keys(ARTS) as ArtName[]) {
      const light = svgOf(artUrl(name, false));
      const dark = svgOf(artUrl(name, true));
      for (const svg of [light, dark]) {
        expect(svg.startsWith("<svg")).toBe(true);
        expect(svg).not.toContain("prefers-color-scheme");
      }
      // The dark colors (the line color #4f5b69) come last in the dark one and are gone
      // from the light one.
      expect(light).not.toContain("#4f5b69");
      expect(dark.indexOf("--lt-line:#4f5b69")).toBeGreaterThan(dark.indexOf("--lt-line:#c3cad4"));
    }
  });
});

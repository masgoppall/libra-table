// public/brand/empty-states as SVG text, keyed by file name (vite.brand.ts).
declare module "virtual:brand-art" {
  const art: Record<string, string>;
  export default art;
}

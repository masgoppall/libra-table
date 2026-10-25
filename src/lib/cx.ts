/** Joins the truthy class names: cx("btn", active && "active") → "btn active". */
export function cx(...names: (string | false | null | undefined)[]): string {
  return names.filter(Boolean).join(" ");
}

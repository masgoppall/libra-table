// Per-viewer conveniences in localStorage (sizes, toggles). Never required to work.

export function readNumber(key: string, fallback: number, min: number, max: number): number {
  try {
    const raw = localStorage.getItem(key);
    const n = raw === null ? NaN : Number(raw);
    return n >= min && n <= max ? n : fallback;
  } catch {
    return fallback;
  }
}

export function writeValue(key: string, value: string | number): void {
  try {
    localStorage.setItem(key, String(value));
  } catch {
    // Private windows and blocked storage: keep going without it.
  }
}

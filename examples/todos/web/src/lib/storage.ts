// Thin localStorage wrapper. JSON-serializes values; swallows any error so
// in-memory state still works in private/headless contexts.

export function load<T>(key: string, fallback: T): T {
  try {
    if (typeof localStorage === "undefined") return fallback;
    const raw = localStorage.getItem(key);
    if (raw == null) return fallback;
    return JSON.parse(raw) as T;
  } catch (_) {
    return fallback;
  }
}

export function save(key: string, value: unknown): void {
  try {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem(key, JSON.stringify(value));
  } catch (_) {
    // ignore — storage quotas, private mode, etc.
  }
}

export interface RecentView {
  id: number;
  name: string;
  at: number;
}

const KEY = "flux-recent-views";
const MAX = 8;

/** 最近浏览：只存 id/名称/时间戳，不存任何敏感数据。
 *  纯 localStorage，隐私模式静默降级；SSR 下返回空数组。 */
export function readRecentViews(): RecentView[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = localStorage.getItem(KEY);
    const arr: unknown = raw ? JSON.parse(raw) : [];
    if (!Array.isArray(arr)) return [];
    return arr
      .filter(
        (x): x is RecentView =>
          !!x &&
          typeof x.id === "number" &&
          Number.isFinite(x.id) &&
          typeof x.name === "string" &&
          typeof x.at === "number",
      )
      .slice(0, MAX);
  } catch {
    return [];
  }
}

export function pushRecentView(id: number, name: string): void {
  if (typeof window === "undefined") return;
  if (!Number.isFinite(id) || id <= 0) return;
  try {
    const next = [
      { id, name: name.slice(0, 120), at: Date.now() },
      ...readRecentViews().filter((v) => v.id !== id),
    ].slice(0, MAX);
    localStorage.setItem(KEY, JSON.stringify(next));
  } catch {
    /* 隐私模式：不记忆，不影响浏览 */
  }
}

export function clearRecentViews(): void {
  if (typeof window === "undefined") return;
  try {
    localStorage.removeItem(KEY);
  } catch {
    /* 同上 */
  }
}

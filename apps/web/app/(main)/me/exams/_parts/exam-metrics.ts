import { formatBytes } from "@/lib/format";

/** 考核页的折算逻辑（从 page.tsx 抽出成纯函数）。
 *
 *  抽出来的两个理由：① 路由页正踩在 300 行门禁上，样图 § exams 要的
 *  「达标率 / 剩余天数」再往里加就顶破；② 这段是「键名 → 单位口径」的
 *  映射，页面里没法便宜地钉，抽出来后三语/后端加键都能用一条用例兜。 */

export interface ExamCurrent {
  uploaded?: number;
  seed_seconds?: number;
  uploads?: number;
  subtitles?: number;
}

/** 指标现值。字节键走 formatBytes，秒键折算小时；
 *  `download_delta` 是累计口径（结算时才判定），一律不展示增量。 */
export function examCurrent(
  key: string,
  cur: ExamCurrent,
  hoursUnit: string,
): string {
  if (key === "upload_delta" && cur.uploaded !== undefined) {
    return formatBytes(cur.uploaded);
  }
  if (key === "download_delta") return "—";
  if (key === "seed_seconds_delta" && cur.seed_seconds !== undefined) {
    return `${(cur.seed_seconds / 3600).toFixed(1)} ${hoursUnit}`;
  }
  if (key === "seed_points_delta" && cur.seed_seconds !== undefined) {
    return `${(cur.seed_seconds / 3600).toFixed(0)}`;
  }
  if (key === "uploads" && cur.uploads !== undefined) {
    return String(cur.uploads);
  }
  if (key === "subtitles" && cur.subtitles !== undefined) {
    return String(cur.subtitles);
  }
  return "—";
}

/** 指标目标值：与现值同一套单位口径，否则「1.2 GB / 4320」这种
 *  一边字节一边小时数的对照会直接读错。 */
export function examTarget(key: string, v: number, hoursUnit: string): string {
  if (key === "upload_delta" || key === "download_delta") {
    return formatBytes(v);
  }
  if (key === "seed_seconds_delta") {
    return `${(v / 3600).toFixed(0)} ${hoursUnit}`;
  }
  return v.toLocaleString();
}

export interface ExamSummary {
  /** 本组达标率（0-100，四舍五入）；空组算 0 而不是 NaN */
  rate: number;
  /** 最近一条未达标项的剩余天数；没有未完成项或没有期限 → null */
  daysLeft: number | null;
}

/** 样图 § exams 顶部那两个指标：达标率 + 「考核剩余 N 天」。
 *  status 非 1（未达标）且带 deadline 的才算进剩余天数；已过期不报负数。 */
export function examSummary(
  rows: { status: number; deadline: string | null }[],
  now: number = Date.now(),
): ExamSummary {
  const total = rows.length;
  const done = rows.filter((e) => e.status === 1).length;
  const rate = total === 0 ? 0 : Math.round((done / total) * 100);
  const open = rows
    .filter((e) => e.status !== 1 && e.deadline)
    .map((e) => new Date(String(e.deadline)).getTime())
    .filter((n) => !Number.isNaN(n));
  const daysLeft = open.length
    ? Math.max(0, Math.ceil((Math.min(...open) - now) / 86400000))
    : null;
  return { rate, daysLeft };
}

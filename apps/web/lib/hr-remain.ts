// H&R 截止日的「剩余 / 逾期」算法（纯函数：组件只负责把天数插进文案）。
//
// 为什么单独一个件：这张表此前只把 deadline 原文打出来，「还剩几天」要
// 用户自己心算；样图 § myhr 的「剩余」列要的正是这个数。算术收在这里，
// `tests/hr-remain.test.ts` 直接钉住边界，组件侧不再各写一份 Date 减法。

/** 一天毫秒数（UTC 日历日；跨夏令时的本地日差不影响「还剩几天」的口径） */
const DAY_MS = 86_400_000;

export interface HrRemain {
  /** deadline 已早于当下（该催办，不是该等待） */
  overdue: boolean;
  /** 天数，向上取整（剩 6 小时算「剩 1 天」），恒 ≥ 1 */
  days: number;
}

/**
 * 距 H&R 截止还剩几天 / 已逾期几天。
 *
 * `now` 由调用方（RSC 渲染时刻）传入并在同一次渲染里复用同一个值：
 * SSR 与注水各取一次 Date.now() 会在跨日界线上算出两个数，React 直接
 * 报 hydration mismatch。deadline 缺失或解析不出（历史脏数据 / null）
 * 返回 null，调用方画「—」，不猜。
 */
export function hrRemain(
  deadline: string | null | undefined,
  now: number,
): HrRemain | null {
  const end = Date.parse(deadline ?? "");
  if (!Number.isFinite(end)) return null;
  const diff = end - now;
  return {
    overdue: diff < 0,
    days: Math.max(1, Math.ceil(Math.abs(diff) / DAY_MS)),
  };
}

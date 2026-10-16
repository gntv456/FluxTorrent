/** 相对时间（同类站口径：发布于 x 天前；完整时间由调用侧放 title）。
 *
 *  这段逻辑原先住在详情页 RSC 的一个闭包里，并把闭包当 prop 往下传。闭包
 *  过不了 Server→Client 边界：React 序列化 props 时直接抛
 *  「Functions cannot be passed directly to Client Components」，整条
 *  /torrent/{id} 落到 error.tsx（2026-10-16 真浏览器实测：容器镜像与 HEAD
 *  同现，digest 3658788688）。抽成 (iso, 文案切片) 纯函数后，服务端与客户端
 *  各算各的，同一句文案两处同口径。 */

import type { Dict } from "@/i18n/zh-CN";

type RelTexts = Pick<
  Dict["tdetail"],
  "justNow" | "hourUnit" | "dayUnit" | "agoUnit"
>;

/** d = dict.tdetail 里那四枚文案 */
export function relTime(iso: string, d: RelTexts): string {
  const ms = Date.now() - new Date(iso).getTime();
  if (ms < 3600000) return d?.justNow;
  const h = Math.floor(ms / 3600000);
  const dd = Math.floor(ms / 86400000);
  if (ms < 86400000) return `${h} ${d?.hourUnit}${d?.agoUnit}`;
  return `${dd} ${d?.dayUnit}${d?.agoUnit}`;
}

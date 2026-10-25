/** 种子表行视觉子件（从 torrent-table.tsx 按域拆出，300 行门禁）：
 *  类型色块 / 封面回退块 / 徽标类名与 tooltip 拼装。
 *  全部纯函数与纯展示，无 hooks、无 i18n 依赖。 */

import { Icon, ICON_NAMES } from "@/components/icons";

/** 已注册图标名（站长可手填 icon_key，未注册的名字不能当图标用） */
const KNOWN_ICONS = new Set<string>(ICON_NAMES as readonly string[]);

/** 促销徽标 tooltip（C2 同类站对标 2026-10-16）：
 *  名 · 剩余 · 绝对结束时刻三段按存在性拼接——「Free 77D」只答剩余，
 *  悬停补「到什么时候」，长期促销不再成谜。 */
export function promoTitle(
  name: string,
  left: string | null,
  endsAt: string | null,
) {
  return [name, left, endsAt].filter(Boolean).join(" · ");
}

/** 促销徽标类名（免费系高亮） */
export const promoCls = (k: string) =>
  k === "free" ? "torrents-promo torrents-promo--free" : "torrents-promo";

/** 审核状态徽标类名：被拒 = 红系变体；暂缓(4) = 中性灰系变体 */
export const approvalCls = (s: number) =>
  s === 2
    ? "torrents-approval torrents-approval--rejected"
    : s === 4
      ? "torrents-approval torrents-approval--held"
      : "torrents-approval";

/** 用户标签徽标内联色（bg_color 缺省回退皮肤默认） */
export const tagStyle = (b: { bg_color?: string; color?: string }) =>
  b.bg_color
    ? { background: b.bg_color, color: b.color || undefined }
    : undefined;

/** 类型色块（0166）：站长手填 icon_key 未注册时 Icon 渲染 null、色块会空，
 *  必须自己判存在，缺失回落分类名首字 */
export function CatBlock({ ico, name, color }: {
  ico?: string;
  name?: string;
  color: string;
}) {
  return (
    <span
      aria-hidden
      className="torrents-cat-block"
      style={{ background: color }}
      title={name ?? ""}
    >
      {ico && KNOWN_ICONS.has(ico) ? (
        <Icon name={ico} size={22} className="torrents-cat-ico" />
      ) : (
        (name ?? "?").slice(0, 1)
      )}
    </span>
  );
}

/** 封面回退块：类型色块底 + 唱片图标（外链海报无图/挂图时） */
export function CoverFallback({ color }: { color: string }) {
  return (
    <span
      className="torrents-cover torrents-cover--fallback"
      style={{ background: color }}
    >
      <Icon name="disc" size={20} />
    </span>
  );
}

/** 存活时长文案（今天 / N 天 / N 个月，同类站口径） */
export function aliveText(
  ageDays: number,
  d: { today: string; days: string; months: string },
) {
  if (ageDays < 1) return d.today;
  if (ageDays < 30) return `${ageDays} ${d.days}`;
  return `${Math.floor(ageDays / 30)} ${d.months}`;
}

/** 促销剩余时间（同类站「6天23时」口径；无截止/已过期返回 null） */
export function remaining(
  end: string | null | undefined,
  now: number,
  fmt: (d: number, h: number) => string,
): string | null {
  if (!end) return null;
  const ms = new Date(end).getTime() - now;
  if (ms <= 0) return null;
  return fmt(Math.floor(ms / 86400000), Math.floor((ms % 86400000) / 3600000));
}

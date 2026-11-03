/** 种子表行视觉子件（从 torrent-table.tsx 按域拆出，300 行门禁）：
 *  类型色块 / 封面回退块 / 徽标类名与 tooltip 拼装。
 *  全部纯函数与纯展示，无 hooks、无 i18n 依赖。 */

import { Icon, ICON_NAMES } from "@/components/icons";
import { CatIcon } from "@/components/cat-icon";

/** 已注册图标名（站长可手填 icon_key，未注册的名字不能当图标用） */
const KNOWN_ICONS = new Set<string>(ICON_NAMES as readonly string[]);

/** 山海经分类色岩彩化（乙）：站点配置的分类色是任意内联色（蓝/粉等），
 *  纯 CSS 无法按「色值」分支重映射——给色块挂一个由颜色字符串稳定散列出的
 *  岩彩色相编号（0 石青 / 1 石绿 / 2 金 / 3 朱砂），
 *  theme-shanhaijing.css 按 [data-shj-hue] 用 !important 覆盖内联背景。
 *  属性恒输出、仅山海经作用域消费，其余三主题渲染零变化。 */
export function shjCatHue(color: string): number {
  let h = 0;
  for (let i = 0; i < color.length; i++) h = (h + color.charCodeAt(i)) % 97;
  return h % 4;
}

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

/** 促销徽标类名（免费系高亮 → 全 kind 类名：山海经印章按 kind 分色；
 *  既有主题只消费 .torrents-promo/--free 两条，新增类名对它们零影响） */
export const promoCls = (k: string) =>
  `torrents-promo torrents-promo--${k}`;

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

/** 类型色块（0166 → 2026-11 位图图标接线）：内层由单色 Icon 换为 CatIcon
 *  （aurora / 山海经 v3.1 位图套，按 <html data-theme> 自动换集）。
 *  色块背景撤除：位图图标自带底砖，与 bg_color 块叠色会双重底色（取舍见
 *  2026-11-03 接线回报）；bg_color 仅保留给缺图时的首字回退。 */
export function CatBlock({ ico, name, color }: {
  ico?: string;
  name?: string;
  color: string;
}) {
  const hasIcon = !!ico && KNOWN_ICONS.has(ico);
  return (
    <span
      aria-hidden
      className="torrents-cat-block"
      style={hasIcon ? undefined : { background: color }}
      data-shj-hue={hasIcon ? undefined : shjCatHue(color)}
      title={name ?? ""}
    >
      {hasIcon ? (
        <CatIcon iconKey={ico} name={name} color={color} size={40} />
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
      data-shj-hue={shjCatHue(color)}
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

import { FilterLinks } from "@/components/ui/filter-links";
import type { Dict } from "@/i18n/server";

/** 我的 H&R 的页面级外围（样图 § myhr 顶部）：KPI 条 + 状态筛选。
 *
 *  与 `components/my-hr.tsx`（表本体，client）分文件：那张表本身就贴着
 *  300 行门禁，再塞统计条与筛选会顶破（按域拆分的既有做法）。
 *  两件事都不需要状态——筛选走链接档（`?status=`），统计由 RSC 传数，
 *  所以本文件保持服务端组件，禁用 JS 也能筛。 */

/** 后端 `?status=` 的白名单（拼错的值一律当「全部」，与后端同源） */
export const HR_STATUSES = [
  "open",
  "satisfied",
  "violated",
  "pardoned",
] as const;

type HrDict = Dict["myhr"];

/** 状态值 → 标签文案（tabAll 给「全部」那一档，不带 value） */
function statusLabel(t: HrDict, v: string): string {
  switch (v) {
    case "open":
      return t.tabOpen;
    case "satisfied":
      return t.tabOk;
    case "violated":
      return t.tabFlagged;
    case "pardoned":
      return t.tabPardoned;
    default:
      return t.tabAll;
  }
}

function num(n: number): string {
  return n.toLocaleString();
}

/** KPI 五格：总数 + 四个状态各一格。
 *  counts 是后端跨状态的全量计数，不能拿当页行数冒充。 */
export function HrKpiStrip({
  t,
  total,
  counts,
}: {
  t: HrDict;
  total: number;
  /** 后端给的四状态全量计数，逐格展示（不拿当页行数算） */
  counts: {
    open: number;
    satisfied: number;
    violated: number;
    pardoned: number;
  };
}) {
  const cells: [string, number][] = [
    [t.kpiTotal, total],
    [t.kpiOpen, counts.open],
    [t.kpiOk, counts.satisfied],
    [t.kpiBad, counts.violated],
    // 样图 § myhr 的第四格「历史豁免」。counts.pardoned 后端一直有
    // （hr_mine.rs:63），此前只喂给筛选器、KPI 条上不报数 ⇒ 成员自助
    // 免罪成功后页面数字毫无变化，像操作没生效。
    [t.tabPardoned, counts.pardoned],
  ];
  return (
    <div className="pgstats">
      {cells.map(([k, v], i) => (
        <div key={`${k}-${i}`} className={i === 3 && v > 0 ? "hl" : undefined}>
          <div className="k">{k}</div>
          <div className="v">{num(v)}</div>
        </div>
      ))}
    </div>
  );
}

/** 五档状态筛选（全部 / 四状态），链接档：换筛选回到第 1 页 */
export function HrStatusFilter({
  t,
  status,
  hrefFor,
}: {
  t: HrDict;
  /** 当前筛选（空串 = 全部） */
  status: string;
  /** 由页面拼 URL：换筛选一律丢 page */
  hrefFor: (v: string) => string;
}) {
  return (
    <FilterLinks
      label={t.colStatus}
      value={status || undefined}
      options={[
        { label: statusLabel(t, ""), href: hrefFor("") },
        ...HR_STATUSES.map((v) => ({
          value: v,
          label: statusLabel(t, v),
          href: hrefFor(v),
        })),
      ]}
    />
  );
}

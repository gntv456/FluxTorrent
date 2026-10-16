import type { Dict } from "@/i18n/server";
import { PANEL_MD } from "@/lib/ui-classes";
import { Badge } from "@/components/ui/badge";

export interface MetricCheck {
  key: string;
  required: number;
  current: number;
  ok: boolean;
}

export interface AssignedPost {
  type_id: number;
  name: string;
  /** 该岗位行所属考核期（本期或可补领的上期） */
  period?: string;
  /** 上期补领行（0106）：claim 时需带 period */
  claimable_prev?: boolean;
  base_pay: number;
  checks: MetricCheck[];
  all_ok: boolean;
  qualified_months: number;
  bonus: number;
  total: number;
  claimed: boolean;
  /** 0106：0=进行中 1=已发薪 2=未达标 3=撤销 */
  status?: number;
}

/** 岗位考核卡（2026-10-16 自 _inner.tsx 按域外移）。
 *
 *  外移动因：_inner.tsx 基线 322 行已超 300 上限（line_limit「存量超限
 *  只许瘦不许胖」），而徽章要从 sticker 色类迁到 Badge 需要净增行数，
 *  先把这张约 70 行的卡片搬出来才腾得出余量。
 *
 *  dict / currency / reqLabel / fmtVal / busyId / onClaim 全部由父组件
 *  注入——本组件不做任何数据拉取与 i18n 取值，纯展示 + 一个领取回调。
 */
export function JixiaoPostCard({
  post: p,
  dict,
  currency,
  reqLabel,
  fmtVal,
  busyId,
  onClaim,
}: {
  post: AssignedPost;
  dict: Dict;
  currency: string;
  /** 指标键 → 中文名（父组件按 dict.jixiao2 组装） */
  reqLabel: Record<string, string>;
  /** 指标值格式化（字节类走 fmtBytes，其余 toLocaleString） */
  fmtVal: (k: string, v: number) => string;
  /** 正在领取中的岗位 id（禁用按钮用） */
  busyId: number | null;
  onClaim: (p: AssignedPost) => void;
}) {
  const t = dict.jixiao2;
  return (
    <article className={PANEL_MD}>
      <header className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="font-display text-lg">
          {p.name}
          {p.claimable_prev && p.period && (
            <span className="ml-2 align-middle text-xs font-normal text-sub">
              （{p.period} 补领）
            </span>
          )}
        </h2>
        <Badge
          tone={p.status === 1 ? "brand" : p.all_ok ? "success" : "warn"}
          variant="soft"
        >
          {p.status === 1
            ? t.stPaid
            : p.status === 2
              ? t.stFailedPeriod
              : p.all_ok
                ? t.stQualified
                : t.stNotQualified}
        </Badge>
      </header>
      <dl className="mt-3 flex flex-col gap-2">
        <div className="flex justify-between">
          <dt className="text-xs text-sub">{t.colBasePay}</dt>
          <dd className="num text-sm font-bold">
            {currency} {p.base_pay.toLocaleString()}
          </dd>
        </div>
        {p.checks.map((c) => (
          <div
            key={c.key}
            className="flex items-center justify-between gap-2"
          >
            <dt className="text-xs text-sub">{reqLabel[c.key] ?? c.key}</dt>
            <dd
              className={`num text-sm ${
                c.ok ? "text-emerald-600" : "text-sub"
              }`}
            >
              {fmtVal(c.key, c.current)} / {fmtVal(c.key, c.required)}
              <span className="ml-1">{c.ok ? "✓" : "✗"}</span>
            </dd>
          </div>
        ))}
        {p.qualified_months > 0 && (
          <div className="flex justify-between">
            <dt className="text-xs text-sub">{t.qualifiedMonths}</dt>
            <dd className="num text-sm">
              {p.qualified_months}（+{currency} {p.bonus.toLocaleString()}）
            </dd>
          </div>
        )}
        <div className="flex justify-between border-t border-line pt-2">
          <dt className="text-xs text-sub">{t.stTotal}</dt>
          <dd className="num text-sm font-bold">
            {currency} {p.total.toLocaleString()}
          </dd>
        </div>
      </dl>
      <button
        className="cmgmt-act cmgmt-act--ok mt-3 w-full"
        disabled={
          p.claimed || p.status === 1 || !p.all_ok || busyId === p.type_id
        }
        onClick={() => onClaim(p)}
      >
        {p.claimed || p.status === 1
          ? t.stClaimed
          : p.all_ok
            ? p.claimable_prev
              ? t.btnClaimPrev
              : t.btnClaim
            : t.stNotQualified}
      </button>
    </article>
  );
}

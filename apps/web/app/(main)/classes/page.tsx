"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** P2 触点 #2：等级要求公开页（0226，UNIT3D /groups/requirements 口径）。
 *  消费 GET /api/v1/classes——门槛八维（0501 前四维+新四维）+ 特权名。
 *  E3 进度卡：启用维度的当前值 vs 下一级门槛达标矩阵 + 差值。 */

interface ClassRow {
  id: number;
  name: string;
  /** 这一档有没有升降级规则行；false = 捐赠/任命发放档（如贵宾），
   *  四维是后端 COALESCE 出来的占位 0，不该出现在晋升阶梯里 */
  has_rule: boolean;
  min_uploaded: number;
  min_download_count: number;
  min_seed_hours: number;
  min_account_age_days: number;
  demotable: boolean;
  /** 0501 四维：null = 不参与判定 */
  min_downloaded: number | null;
  min_seed_volume: number | null;
  min_seed_count: number | null;
  min_ratio_bp: number | null;
}
interface ClassesData {
  me_class: number;
  me_stats: {
    uploaded: number;
    download_count: number;
    seed_hours: number;
    account_age_days: number;
    downloaded: number;
    seed_volume: number;
    seed_count: number;
    ratio_bp: number;
  } | null;
  classes: ClassRow[];
  staff_classes: { id: number; name: string }[];
}

/** 四维键序：与表格列、达标判定一一对应 */
const DIMS = [
  "uploaded",
  "download_count",
  "seed_hours",
  "account_age_days",
  "downloaded",
  "seed_volume",
  "seed_count",
  "ratio_bp",
] as const;
type Dim = (typeof DIMS)[number];

function fmtBytes(n: number): string {
  if (n <= 0) return "—";
  const gb = n / 1024 / 1024 / 1024;
  if (gb >= 1024) return `${(gb / 1024).toFixed(0)} TB`;
  return `${gb >= 100 ? gb.toFixed(0) : gb.toFixed(1)} GB`;
}
const fmtNum = (n: number) => (n > 0 ? n.toLocaleString() : "—");
/** 0501 可空门槛：null = 未启用该维度 */
const fmtOptBytes = (n: number | null) => (n === null ? "—" : fmtBytes(n));
const fmtOptRatio = (bp: number | null) =>
  bp === null || bp <= 0 ? "—" : (bp / 10000).toFixed(2);
function fmtVal(dim: Dim, n: number): string {
  if (dim === "uploaded" || dim === "downloaded" || dim === "seed_volume")
    return fmtBytes(n);
  if (dim === "ratio_bp")
    return n >= Number.MAX_SAFE_INTEGER ? "∞" : (n / 10000).toFixed(2);
  if (dim === "seed_count") return fmtNum(n);
  if (dim === "seed_hours") return n > 0 ? `${n.toLocaleString()} h` : "—";
  if (dim === "account_age_days")
    return n > 0 ? `${n.toLocaleString()} 天` : "—";
  return fmtNum(n);
}
/** 差值文案：已达标本地表 NULL，由调用方渲染达标标 */
function fmtGap(dim: Dim, need: number): string {
  if (dim === "uploaded" || dim === "downloaded" || dim === "seed_volume") {
    const gb = need / 1024 / 1024 / 1024;
    return gb >= 1024
      ? `还差 ${(gb / 1024).toFixed(1)} TB`
      : `还差 ${gb >= 100 ? gb.toFixed(0) : gb.toFixed(1)} GB`;
  }
  if (dim === "seed_hours") return `还差 ${need.toLocaleString()} h`;
  if (dim === "account_age_days")
    return `还差 ${need.toLocaleString()} 天（约 ${Math.ceil(need / 30)} 个月）`;
  return `还差 ${need.toLocaleString()} 个`;
}

/** 我的进度卡：下一级四维达标矩阵 */
function ProgressCard({ d }: { d: ClassesData }) {
  const s = d.me_stats;
  if (!s) return null;
  const next = d.classes.find((c) => c.id === d.me_class + 1);
  const cur = d.classes.find((c) => c.id === d.me_class);
  if (!next) {
    return (
      <div className="brand-panel mt-4 p-4 text-sm">
        🎉 你已达成长长线最高等级（
        {cur?.name ?? `LV${d.me_class}`}）。做种与分享本身就是荣誉的下一站。
      </div>
    );
  }
  const mine: Record<Dim, number> = {
    uploaded: s.uploaded,
    download_count: s.download_count,
    seed_hours: s.seed_hours,
    account_age_days: s.account_age_days,
    downloaded: s.downloaded,
    seed_volume: s.seed_volume,
    seed_count: s.seed_count,
    ratio_bp: s.ratio_bp >= 2147483647
      ? Number.MAX_SAFE_INTEGER
      : s.ratio_bp,
  };
  const need: Record<Dim, number | null> = {
    uploaded: next.min_uploaded,
    download_count: next.min_download_count,
    seed_hours: next.min_seed_hours,
    account_age_days: next.min_account_age_days,
    // 0501 四维：null = 站点未启用，不进达标矩阵
    downloaded: next.min_downloaded,
    seed_volume: next.min_seed_volume,
    seed_count: next.min_seed_count,
    ratio_bp: next.min_ratio_bp,
  };
  const active = DIMS.filter((k) => need[k] !== null);
  const met = active.filter((k) => mine[k] >= (need[k] as number)).length;
  return (
    <div className="brand-panel mt-4 p-4">
      <div className="flex items-baseline justify-between">
        <p className="text-sm font-medium">
          我的进度：{cur?.name ?? `LV${d.me_class}`} → {next.name}
        </p>
        <p className="text-xs text-sub">
          {met}/{active.length} 维达标
        </p>
      </div>
      <div className="mt-3 grid gap-3 sm:grid-cols-2">
        {active.map((k) => {
          const nk = need[k] as number;
          const ok = mine[k] >= nk;
          const pct =
            nk > 0 ? Math.min(100, (mine[k] / nk) * 100) : 100;
          return (
            <div key={k} className="rounded-lg border border-line/50 p-3">
              <div className="flex items-center justify-between text-xs">
                <span className="text-sub">
                  {k === "uploaded"
                    ? "上传量"
                    : k === "download_count"
                      ? "完成数"
                      : k === "seed_hours"
                        ? "做种时长"
                        : k === "account_age_days"
                          ? "账龄"
                          : k === "downloaded"
                            ? "下载量"
                            : k === "seed_volume"
                              ? "做种体积"
                              : k === "seed_count"
                                ? "做种数"
                                : "分享率"}
                </span>
                <span className={ok ? "text-accent" : "text-sub"}>
                  {ok ? "✓ 达标" : fmtGap(k, nk - mine[k])}
                </span>
              </div>
              <progress
                className="mt-2 h-1.5 w-full accent-[var(--compat)]"
                value={pct}
                max={100}
              />
              <p className="mt-1 font-mono text-xs text-sub">
                {fmtVal(k, mine[k])} / {fmtVal(k, nk)}
              </p>
            </div>
          );
        })}
      </div>
      <p className="mt-3 text-xs text-sub">
        {met === active.length
          ? "全部维度已达标，等级调整由后台任务自动完成（分钟级），无需申请。"
          : "全部达标即自动晋升，无需申请；「可降级」的等级跌破门槛会自动降回。"}
      </p>
    </div>
  );
}

export default function ClassesPage() {
  const { dict } = useI18n();
  void dict;
  const [d, setD] = useState<ClassesData | null>(null);
  const [err, setErr] = useState("");

  useEffect(() => {
    api
      .get<ClassesData>("/api/v1/classes")
      .then(setD)
      .catch((e) => setErr(String(e)));
  }, []);

  if (err) return <p className="py-8 text-center text-[var(--danger-text)]">{err}</p>;
  if (!d) return <p className="py-8 text-center text-sub">…</p>;

  // 阶梯只画有规则行的档；贵宾一类「捐赠/任命发放」档另起一句说明，
  // 否则成员在自动晋升表里看到一排破折号，读到的是「贵宾零门槛」
  const ladder = d.classes.filter((c) => c.has_rule);
  const granted = d.classes.filter((c) => !c.has_rule);

  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <h1 className="font-display text-xl">等级要求</h1>
      <p className="mt-1 text-xs text-sub">
        全部达标自动晋升（分钟级生效，无需申请）；「可降级」的等级跌破门槛会自动降回。
      </p>
      <ProgressCard d={d} />
      <div className="brand-panel mt-4 overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-line text-left text-xs text-sub">
              <th className="px-3 py-2">等级</th>
              <th className="px-3 py-2">上传量</th>
              <th className="px-3 py-2">完成数</th>
              <th className="px-3 py-2">做种时长</th>
              <th className="px-3 py-2">账龄</th>
              <th className="px-3 py-2">下载量</th>
              <th className="px-3 py-2">做种体积</th>
              <th className="px-3 py-2">做种数</th>
              <th className="px-3 py-2">分享率</th>
              <th className="px-3 py-2">降级</th>
            </tr>
          </thead>
          <tbody>
            {ladder.map((c) => (
              <tr
                key={c.id}
                className={`border-b border-line/50 ${
                  c.id === d.me_class ? "bg-accent/10 font-medium" : ""
                }`}
              >
                <td className="px-3 py-2">
                  {c.name}
                  {c.id === d.me_class && (
                    <span className="ml-2 text-xs text-accent">当前</span>
                  )}
                </td>
                <td className="px-3 py-2 font-mono">
                  {fmtBytes(c.min_uploaded)}
                </td>
                <td className="px-3 py-2 font-mono">
                  {fmtNum(c.min_download_count)}
                </td>
                <td className="px-3 py-2 font-mono">
                  {fmtVal("seed_hours", c.min_seed_hours)}
                </td>
                <td className="px-3 py-2 font-mono">
                  {fmtVal("account_age_days", c.min_account_age_days)}
                </td>
                <td className="px-3 py-2 font-mono">
                  {fmtOptBytes(c.min_downloaded)}
                </td>
                <td className="px-3 py-2 font-mono">
                  {fmtOptBytes(c.min_seed_volume)}
                </td>
                <td className="px-3 py-2 font-mono">
                  {fmtNum(c.min_seed_count ?? 0)}
                </td>
                <td className="px-3 py-2 font-mono">
                  {fmtOptRatio(c.min_ratio_bp)}
                </td>
                <td className="px-3 py-2 text-xs">
                  {c.demotable ? "可降级" : "—"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {granted.length > 0 && (
        <p className="mt-3 text-xs text-sub">
          授予等级（捐赠/任命发放，不进自动晋升线）：
          {granted.map((g) => g.name).join(" / ")}
        </p>
      )}
      {d.staff_classes.length > 0 && (
        <p className="mt-3 text-xs text-sub">
          职务等级（站长任命）：
          {d.staff_classes.map((s) => s.name).join(" / ")}
        </p>
      )}
    </div>
  );
}

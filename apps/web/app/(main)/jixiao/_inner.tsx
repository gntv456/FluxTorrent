"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { PageHeader } from "@/components/ui/page-header";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import { useIsCompact } from "@/lib/hooks/use-media";
import {
  JixiaoPostCard,
  type AssignedPost,
} from "./_parts/jixiao-post-card";

interface JixiaoMe {
  period: string;
  metrics: Record<string, number>;
  /** 月末结算后的补领窗口天数（0106） */
  claim_window_days?: number;
  assigned: AssignedPost[];
}

interface MyClaim {
  id: number;
  type_name: string;
  period: string;
  amount: number;
  bonus_paid?: number;
  status?: number;
  /** worker=月末自动结算 / self=本人领取 */
  source?: string | null;
  claimed_at: string;
}

/** 绩效考核（NP jixiao.php 口径 + 0106 工作组考核）：
 *  岗位卡片（实时指标 vs 要求 + 达标状态 + 结算状态徽章 + 连续达标加成进度）
 *  + 月末自动结算/补领窗口提示 + 领取记录（自动发放/手动领取标记）。未被分配岗位时给引导文案。 */
export default function JixiaoPage() {
  const { dict, locale, currency } = useI18n();
  const compact = useIsCompact();
  const t = dict.jixiao2;
  const [me, setMe] = useState<JixiaoMe | null>(null);
  const [claims, setClaims] = useState<MyClaim[]>([]);
  const [failed, setFailed] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<number | null>(null);

  const load = useCallback(() => {
    // 成功分支不回写 failed：另一路失败时不能被这一路的成功洗白
    setFailed(false);
    api
      .get<JixiaoMe>("/api/v1/jixiao/me")
      .then(setMe)
      .catch(() => {
        setMe(null);
        setFailed(true);
      });
    api
      .get<MyClaim[]>("/api/v1/jixiao/my")
      .then(setClaims)
      .catch(() => {
        setClaims([]);
        setFailed(true);
      });
  }, []);
  useEffect(load, [load]);

  function flash(m: string) {
    setMsg(m);
    setErr(null);
    setTimeout(() => setMsg(null), 3500);
  }
  function flashErr(m: string) {
    setErr(m);
    setMsg(null);
    setTimeout(() => setErr(null), 5000);
  }

  const reqLabel: Record<string, string> = {
    uploaded: t.mUploaded,
    downloaded: t.mDownloaded,
    uploads: t.mUploads,
    seeding_count: t.mSeeding,
    seed_size: t.mSeedSize,
    ops: t.mOps,
    seed_days: t.mSeedDays,
    seed_hours: t.mSeedHours,
    seed_size_tb: t.mSeedSize,
  };
  const fmtVal = (k: string, v: number) =>
    k === "uploaded" || k === "downloaded" || k === "seed_size"
      ? fmtBytes(v)
      : k === "seed_size_tb"
        ? `${v.toLocaleString()} TB`
        : v.toLocaleString();

  async function claim(p: AssignedPost) {
    setBusyId(p.type_id);
    try {
      const r = await api.post<{ total: number }>("/api/v1/jixiao/claim", {
        type_id: p.type_id,
        ...(p.claimable_prev && p.period ? { period: p.period } : {}),
      });
      flash(
        t.claimed
          .replace("{n}", r.total.toLocaleString())
          .replaceAll("{magic}", currency),
      );
      load();
    } catch (e) {
      flashErr(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusyId(null);
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow="Performance"
        title={t.title}
        sub={t.subtitle}
      />
      {msg && (
        <p className="rounded-[var(--r-md)] bg-mint/30 p-3 text-sm text-ink">{msg}</p>
      )}
      {err && <p className="rounded-[var(--r-md)] bg-sun/30 p-3 text-sm text-ink">{err}</p>}
      {failed && (
        <p className="rounded-[var(--r-md)] bg-sun/30 p-3 text-sm text-ink">
          {dict.common.loadFailed}
        </p>
      )}

      {/* 我的岗位卡片（NP「当前考核信息 + 考核数据统计」合并视图） */}
      {!failed &&
        (me && me.assigned.length > 0 ? (
        <div className="grid gap-3 md:grid-cols-2">
          {me.assigned.map((p) => (
            <JixiaoPostCard
              key={p.type_id}
              post={p}
              dict={dict}
              currency={currency}
              reqLabel={reqLabel}
              fmtVal={fmtVal}
              busyId={busyId}
              onClaim={claim}
            />
          ))}
        </div>
      ) : (
          <p className="rounded-[var(--r-md)] bg-sky-soft p-4 text-sm text-ink">
            {t.noAssignment}
          </p>
        ))}

      {/* 月末自动结算说明（0106）：提示结算机制与补领窗口 */}
      {!failed && me && me.assigned.length > 0 && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-xs text-ink">
          {t.autoSettleNote?.replace("{n}", String(me.claim_window_days ?? 7)) ?? t.note}
        </p>
      )}

      {/* 领取记录（M6.2：compact 档转行卡片） */}
      {!failed && (
      <section className="brand-panel">
        <div className="brand-panel__head">
          <h2>{t.myTitle}</h2>
        </div>
        {compact && claims.length > 0 && (
          <div className="trow-list p-2">
            {claims.map((c) => (
              <div className="claimcard" key={c.id}>
                <p className="claimcard__hd">
                  <span className="claimcard__name">{c.type_name}</span>
                  <span className="num font-bold">
                    +{c.amount.toLocaleString()}
                  </span>
                </p>
                <p className="claimcard__meta num">
                  <span>{c.period}</span>
                  <span>
                    {c.source === "worker"
                      ? t.paidByWorker
                      : t.paidBySelf}
                  </span>
                  <span>
                    {new Date(c.claimed_at).toLocaleString(
                      dateLocale(locale),
                    )}
                  </span>
                </p>
              </div>
            ))}
          </div>
        )}
        {(!compact || claims.length === 0) && (
          <table className={`nexus-table${compact && claims.length > 0 ? " sr-only" : ""}`}>
            <tbody>
              {compact && (
                <tr>
                  <td colSpan={5} className="py-6 text-center text-sub">
                    {t.myEmpty}
                  </td>
                </tr>
              )}
              {!compact && (
                <>
                  <tr>
                    <td className="colhead">{t.colName}</td>
                    <td className="colhead">{t.colPeriod}</td>
                    <td className="colhead">{t.colAmount}</td>
                    <td className="colhead">{t.colPaidBy}</td>
                    <td className="colhead">{t.colAt}</td>
                  </tr>
                  {claims.map((c) => (
                    <tr key={c.id}>
                      <td>{c.type_name}</td>
                      <td className="num">{c.period}</td>
                      <td className="num font-bold">
                        +{c.amount.toLocaleString()}
                      </td>
                      <td className="text-xs">
                        {c.source === "worker"
                          ? t.paidByWorker
                          : t.paidBySelf}
                      </td>
                      <td className="text-xs text-sub">
                        {new Date(c.claimed_at).toLocaleString(
                          dateLocale(locale),
                        )}
                      </td>
                    </tr>
                  ))}
                  {claims.length === 0 && (
                    <tr>
                      <td colSpan={5} className="py-6 text-center text-sub">
                        {t.myEmpty}
                      </td>
                    </tr>
                  )}
                </>
              )}
            </tbody>
          </table>
        )}
      </section>
      )}

      <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-xs text-ink">{t.note}</p>
    </div>
  );
}

function fmtBytes(n: number): string {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(2)} ${units[i]}`;
}

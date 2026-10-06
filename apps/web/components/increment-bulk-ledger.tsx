"use client";

/**
 * 发放台账（0291）。
 *
 * 为什么要有这一页：批量发放过去只在 `audit_log` 里留一条 ref，而 ref 只带
 * **前 20 个目标 id 抽样**（increment_bulk.rs 的 sample_targets）。站长问
 * 「昨天那批 10TB 发到谁了、有没有漏发」，界面上没有答案；`?tool=audit`
 * 又硬顶 200 行且不渲染 ref。发放管理的最低要求是**可按批次回放受众**，
 * 以及看得见「这批发成了没」（status：pending/done/partial/failed）。
 */

import { useCallback, useEffect, useState } from "react";

import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface GrantRow {
  id: number;
  batch_id: string;
  actor_id: number;
  actor_name: string;
  kind: string;
  amount: number;
  item_name: string | null;
  medal_name: string | null;
  targets: number;
  affected: number;
  status: string;
  error: string | null;
  subject: string | null;
  created_at: string;
  target_ids?: number[];
}

interface LedgerProps {
  /** kind → 可读名，沿用批量发放表单那套标签（不另建第二份词表） */
  kindLabel: (k: string) => string;
  /** 发放成功后自增：台账只在 mount 时取过一次数的话，站长发完这一屏还是空的
   *  （实测：必须手点「刷新」才看到刚发的那批），而「发完看不到」正是台账存在的理由 */
  tick?: number;
}

const btn = "min-h-[30px] rounded-full border border-line px-3 text-xs";

export function BulkLedger({ kindLabel, tick = 0 }: LedgerProps) {
  const t = useI18n().dict.adminBulk;
  const [rows, setRows] = useState<GrantRow[]>([]);
  const [open, setOpen] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");

  const load = useCallback(async () => {
    setBusy(true);
    try {
      const r = await api.get<{ rows: GrantRow[] }>(
        "/api/v1/admin/grants?per_page=20",
      );
      setRows(r.rows ?? []);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.lgFail);
    } finally {
      setBusy(false);
    }
  }, [t.lgFail]);

  useEffect(() => {
    void load();
  }, [load, tick]);

  /** 展开某一批：受众清单按需再取（一次 20 批全量拉回太重） */
  async function toggle(batch: string) {
    if (open === batch) {
      setOpen(null);
      return;
    }
    setOpen(batch);
    setBusy(true);
    try {
      const r = await api.get<{ rows: GrantRow[] }>(
        `/api/v1/admin/grants?batch=${encodeURIComponent(batch)}`,
      );
      const hit = (r.rows ?? [])[0];
      if (hit) setRows((prev) => prev.map((x) => (x.id === hit.id ? hit : x)));
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.lgFail);
    } finally {
      setBusy(false);
    }
  }

  const statusLabel = (s: string) =>
    s === "done"
      ? t.lgDone
      : s === "partial"
        ? t.lgPartial
        : s === "failed"
          ? t.lgFailed
          : t.lgPending;

  return (
    <section className="baozi-panel p-4">
      <div className="mb-2 flex items-center gap-2">
        <h3 className="text-sm font-bold">{t.lgTitle}</h3>
        <button className={btn} disabled={busy} onClick={() => void load()}>
          {t.lgRefresh}
        </button>
        {msg && <span className="text-xs text-red-600">{msg}</span>}
      </div>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">{t.lgThTime}</td>
            <td className="colhead">{t.lgThActor}</td>
            <td className="colhead">{t.lgThWhat}</td>
            <td className="colhead">{t.lgThTargets}</td>
            <td className="colhead">{t.lgThAffected}</td>
            <td className="colhead">{t.lgThStatus}</td>
            <td className="colhead text-right">{t.lgThAction}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((g) => (
            <tr key={g.batch_id}>
              <td>{g.created_at.slice(0, 16).replace("T", " ")}</td>
              <td>
                <a
                  href={`/admin/users/${g.actor_id}`}
                  className="font-bold text-link"
                >
                  {g.actor_name}
                </a>
              </td>
              <td>
                {kindLabel(g.kind)} {g.amount}
                {g.item_name ? ` · ${g.item_name}` : ""}
                {g.medal_name ? ` · ${g.medal_name}` : ""}
              </td>
              <td className="num">{g.targets}</td>
              <td className="num">{g.affected}</td>
              <td>
                {statusLabel(g.status)}
                {g.error ? ` · ${g.error}` : ""}
              </td>
              <td className="text-right">
                <button
                  className={btn}
                  disabled={busy}
                  onClick={() => void toggle(g.batch_id)}
                >
                  {open === g.batch_id ? t.lgHide : t.lgView}
                </button>
              </td>
            </tr>
          ))}
          {open &&
            (() => {
              const g = rows.find((x) => x.batch_id === open);
              if (!g) return null;
              return (
                <tr>
                  <td colSpan={7} className="break-all text-sub">
                    {t.lgTargets}:{" "}
                    {(g.target_ids ?? []).join(", ") || t.lgEmpty}
                  </td>
                </tr>
              );
            })()}
          {rows.length === 0 && (
            <tr>
              <td colSpan={7} className="py-4 text-center text-sub">
                {t.lgEmpty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}

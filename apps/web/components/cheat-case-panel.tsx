"use client";

import { useCallback, useEffect, useState, Fragment } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { CheatCaseDetail } from "./cheat-case-detail";

/** 案件行（GET /admin/cheat-cases，0367 M1） */
interface CaseRow {
  id: number;
  user_id: number;
  username: string;
  priority: number;
  risk_score: number | null;
  status: number;
  claimed_by: number | null;
  claimed_by_name: string | null;
  created_at: string;
  sla_breached: boolean;
  open_evidence: number;
  total_hits: number;
}

type TabKey = "open" | "claimed" | "done";

/**
 * 反作弊案件队列（0367 M1）：队列 / 认领互斥 / 详情展开（证据+备注）。
 * 案件长在 reports 表（ref_type='cheat'），本面板是 staff 协同工作面。
 */
export function CheatCasePanel() {
  const { dict, locale } = useI18n();
  const t = dict.cheaterbox;
  const [tab, setTab] = useState<TabKey>("open");
  const [rows, setRows] = useState<CaseRow[] | null>(null);
  const [busy, setBusy] = useState<number | null>(null);
  const [openId, setOpenId] = useState<number | null>(null);

  const load = useCallback((status: TabKey) => {
    setRows(null);
    api
      .get<{ items: CaseRow[] }>(
        `/api/v1/admin/cheat-cases?status=${status}&limit=100`,
      )
      .then((r) => setRows(r.items))
      .catch(() => setRows([]));
  }, []);

  useEffect(() => {
    load(tab);
  }, [tab, load]);

  const claim = async (id: number, release: boolean) => {
    setBusy(id);
    try {
      await api.post(`/api/v1/admin/cheat-cases/${id}/claim`, { release });
    } catch {
      /* 并发互斥失败等：刷新队列反映真实状态 */
    } finally {
      setBusy(null);
      load(tab);
    }
  };

  const priLabel = (p: number) =>
    p >= 2 ? t.priCritical : p === 1 ? t.priUrgent : t.priNormal;

  const th = "px-3 py-2 text-left text-xs font-bold whitespace-nowrap";
  const td = "px-3 py-2 text-sm align-middle";
  const tabBtn = (k: TabKey) =>
    "rounded-full border px-3 py-1 text-xs font-bold " +
    (tab === k
      ? "border-[var(--baozi-orange)] " +
        "text-[var(--baozi-orange-dark)]"
      : "border-[var(--border-soft)] text-sub");

  return (
    <section className="baozi-panel mt-4">
      <header className="baozi-panel__head">
        <h2>
          <span aria-hidden="true">⚖️</span> {t.casesTitle}
        </h2>
      </header>
      <p className="mb-2 text-xs text-sub">{t.casesNote}</p>

      {/* 状态页签 */}
      <div className="mb-2 flex gap-2">
        {(["open", "claimed", "done"] as TabKey[]).map((k) => (
          <button
            key={k}
            type="button"
            className={tabBtn(k)}
            onClick={() => setTab(k)}
          >
            {k === "open"
              ? t.tabOpen
              : k === "claimed"
                ? t.tabClaimed
                : t.tabDone}
          </button>
        ))}
      </div>

      {rows === null ? (
        <p className="funbox__empty">…</p>
      ) : rows.length === 0 ? (
        <p className="funbox__empty">{t.casesEmpty}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full border-collapse">
            <thead>
              <tr className="border-b border-[var(--border-soft)]">
                <th className={th}>{t.colCaseId}</th>
                <th className={th}>{t.colEventUser}</th>
                <th className={th}>{t.colCasePriority}</th>
                <th className={th}>{t.colCaseRisk}</th>
                <th className={th}>{t.colCaseEvidence}</th>
                <th className={th}>{t.colEventHits}</th>
                <th className={th}>{t.colEventTime}</th>
                <th className={th}>{t.colCaseSla}</th>
                <th className={th} aria-hidden="true" />
              </tr>
            </thead>
            <tbody>
              {rows.map((c) => (
                <Fragment key={c.id}>
                  <tr
                    className={
                      "border-b border-dashed " +
                      "border-[var(--border-soft)]"
                    }
                  >
                    <td className={td}>#{c.id}</td>
                    <td className={td}>
                      <a
                        className={
                          "font-bold " +
                          "text-[var(--baozi-orange-dark)] " +
                          "hover:underline"
                        }
                        href={`/users/${c.user_id}`}
                      >
                        {c.username}
                      </a>
                    </td>
                    <td className={td}>
                      <span
                        className={
                          c.priority >= 2
                            ? "font-bold text-[var(--baozi-red,#c0392b)]"
                            : c.priority === 1
                              ? "text-[var(--baozi-orange-dark)]"
                              : "text-sub"
                        }                      >
                        {priLabel(c.priority)}
                      </span>
                    </td>
                    {/* 风险分快照（0368 M2：开案时固化；跨档自动开案的依据） */}
                    <td className={`${td} whitespace-nowrap`}>
                      {c.risk_score === null ? (
                        <span className="text-sub">—</span>
                      ) : (
                        <span className="font-bold">{c.risk_score}</span>
                      )}
                    </td>
                    <td className={td}>{c.open_evidence}</td>
                    <td className={td}>{c.total_hits}</td>
                    <td
                      className={
                        `${td} whitespace-nowrap text-xs ` +
                        "text-[var(--text-faint)]"
                      }
                    >
                      {new Date(c.created_at).toLocaleString(locale)}
                    </td>
                    <td className={`${td} whitespace-nowrap text-xs`}>
                      {c.sla_breached ? (
                        <span
                          className={
                            "font-bold " +
                            "text-[var(--baozi-red,#c0392b)]"
                          }
                        >
                          {t.slaBreached}
                        </span>
                      ) : (
                        <span className="text-sub">{t.slaOk}</span>
                      )}
                    </td>
                    <td className={`${td} whitespace-nowrap`}>
                      <button
                        type="button"
                        className="text-xs text-sub hover:underline"
                        disabled={busy === c.id}
                        onClick={() =>
                          setOpenId(openId === c.id ? null : c.id)
                        }
                      >
                        {openId === c.id ? t.collapse : t.detail}
                      </button>
                      {c.status === 0 && (
                        <button
                          type="button"
                          className={
                            "ml-2 text-xs font-bold " +
                            "text-[var(--baozi-orange-dark)] " +
                            "hover:underline"
                          }
                          disabled={busy === c.id}
                          onClick={() => void claim(c.id, false)}
                        >
                          {busy === c.id ? "…" : t.claim}
                        </button>
                      )}
                      {c.status === 2 && (
                        <button
                          type="button"
                          className="ml-2 text-xs text-sub hover:underline"
                          disabled={busy === c.id}
                          onClick={() => void claim(c.id, true)}
                        >
                          {busy === c.id ? "…" : t.release}
                        </button>
                      )}
                    </td>
                  </tr>
                  {openId === c.id && (
                    <tr>
                      <td colSpan={9} className="bg-[var(--bg-soft)] px-3 py-2">
                        <CheatCaseDetail
                          caseId={c.id}
                          busy={busy}
                          setBusy={setBusy}
                        />
                      </td>
                    </tr>
                  )}
                </Fragment>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

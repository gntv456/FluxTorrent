"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { toast } from "@/components/ui/toast";
import { useI18n, apiErrorMessage } from "@/i18n/client";

/** 案件详情（GET /admin/cheat-cases/{id}，0367 M1） */
interface CaseDetail {
  id: number;
  username: string;
  reason: string;
  priority: number;
  risk_score: number | null;
  status: number;
  claimed_by_name: string | null;
  created_at: string;
  sla_breached: boolean;
  sla_hours: number;
  evidence: {
    id: number;
    agent: string;
    peer_ip: string | null;
    reason: string;
    hits: number;
    last_seen: string;
    resolved_at: string | null;
  }[];
  notes: { id: number; author: string; body: string; created_at: string }[];
}

/**
 * 案件详情展开区（0367 M1）：开案理由 + 证据列表 + 处理记录时间线 +
 * 协同备注输入。从 cheat-case-panel 拆出（300 行门禁）。
 */
export function CheatCaseDetail({
  caseId,
  busy,
  setBusy,
}: {
  caseId: number;
  busy: number | null;
  setBusy: (n: number | null) => void;
}) {
  const { dict, locale } = useI18n();
  const t = dict.cheaterbox;
  const [detail, setDetail] = useState<CaseDetail | null>(null);
  const [noteText, setNoteText] = useState("");

  const fetchDetail = () =>
    api
      .get<CaseDetail>(`/api/v1/admin/cheat-cases/${caseId}`)
      .then(setDetail)
      .catch(() => setDetail(null));

  // 挂载即拉取（父组件仅在展开时渲染本组件）
  useEffect(() => {
    void fetchDetail();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const submitNote = async () => {
    const body = noteText.trim();
    if (!body) return;
    setBusy(caseId);
    try {
      await api.post(`/api/v1/admin/cheat-cases/${caseId}/note`, { body });
      setNoteText("");
      await fetchDetail();
    } catch (e) {
      // 备注失败保留原文，稍后重试
      toast.error(apiErrorMessage(dict, e));
    } finally {
      setBusy(null);
    }
  };

  if (detail === null) return <p className="funbox__empty">…</p>;

  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm">
        <b>{t.caseReason}：</b>
        {detail.reason}
        {detail.claimed_by_name && (
          <span className="text-sub">
            {t.claimedBy}
            {detail.claimed_by_name}
          </span>
        )}
      </p>
      <h4 className="text-xs font-bold">
        {t.evidenceTitle}（{detail.evidence.length}）
      </h4>
      <ul className="flex flex-col gap-1 text-xs">
        {detail.evidence.map((e) => (
          <li key={e.id} className="flex flex-wrap gap-2">
            <code className="rounded bg-[var(--brand-paper)] px-1">
              {e.agent}
            </code>
            <span>{e.reason}</span>
            <span className="text-sub">
              ×{e.hits}
              {e.resolved_at ? `（${t.resolvedAt}）` : ""}
            </span>
          </li>
        ))}
      </ul>
      <h4 className="text-xs font-bold">
        {t.notesTitle}（{detail.notes.length}）
      </h4>
      <ul className="flex flex-col gap-1 text-xs">
        {detail.notes.map((n) => (
          <li key={n.id}>
            <b>{n.author}</b>
            <span className="text-sub">
              {new Date(n.created_at).toLocaleString(locale)}
            </span>
            <br />
            {n.body}
          </li>
        ))}
      </ul>
      <div className="flex gap-2">
        <input
          className={
            "min-h-[36px] flex-1 rounded-[var(--r-sm)] border " +
            "border-[var(--brand-line)] bg-[var(--brand-paper)] " +
            "px-2 text-sm"
          }
          placeholder={t.notePh}
          value={noteText}
          onChange={(e) => setNoteText(e.target.value)}
        />
        <button
          type="button"
          className="brand-button disabled:opacity-50"
          disabled={busy === detail.id || !noteText.trim()}
          onClick={() => void submitNote()}
        >
          {t.noteSubmit}
        </button>
      </div>
    </div>
  );
}

"use client";

/**
 * 自动审种台账面板（0400/0401）：四指标卡 + 台账列表 + 回放/套用操作。
 * 挂在管理台 `?tool=autoreview`（admin-tool-switch 直排分流）。
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

type Hit = { rule: string; action: string; detail: string };
type LogRow = {
  id: number;
  torrent_id: number;
  name: string;
  hits: Hit[];
  score: number | null;
  verdict: number;
  created_at: string;
};
type Stats = {
  shadow: {
    samples: number;
    agree: number;
    agree_pct: number;
    false_deny: number;
    false_pass: number;
    min_samples: number;
    min_agree_pct: number;
    ready: boolean;
  };
  today: { pass: number; deny: number; postpone: number };
  pending_human: number;
  rule_health: [string, number, number][];
};

const CARD =
  "rounded-[var(--r-md)] border border-line bg-white p-3 text-sm text-ink";


export function AutoReviewPanel() {
  const { dict } = useI18n();
  // autoReview 段已入字典（三语齐全），直接用强类型键，不再留 CJK 兜底
  const ar = dict.admin.autoReview;
  const t = (k: keyof typeof ar): string => ar[k];
  const VERDICT: Record<number, string> = {
    0: ar.verdictPass,
    1: ar.verdictDeny,
    2: ar.verdictPostpone,
    3: ar.verdictDeep,
  };
  const [stats, setStats] = useState<Stats | null>(null);
  const [rows, setRows] = useState<LogRow[]>([]);
  const [rule, setRule] = useState("");
  const [msg, setMsg] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const s = await api.get<Stats>("/admin/auto-review/stats");
      setStats(s);
      const q = rule
        ? `?rule=${encodeURIComponent(rule)}&limit=50`
        : "?limit=50";
      const l = await api.get<{ items: LogRow[] }>(
        `/admin/auto-review/logs${q}`,
      );
      setRows(l.items ?? []);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : ar.loadFailed);
    }
  }, [rule]);

  useEffect(() => {
    void load();
  }, [load]);

  const replay = async () => {
    setBusy(true);
    try {
      const r = await api.post<{ replayed: number; changed: number }>(
        "/admin/auto-review/replay",
        {},
      );
      setMsg(
        ar.replaySummary
            .replace("{n}", String(r.replayed))
            .replace("{same}", String(r.replayed - r.changed))
            .replace("{changed}", String(r.changed)),
      );
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : ar.replayFailed);
    } finally {
      setBusy(false);
    }
  };

  const restore = async () => {
    if (!rule) {
      setMsg(ar.pickRuleFirst);
      return;
    }
    setBusy(true);
    try {
      const r = await api.post<{ restored: number; scanned: number }>(
        "/admin/auto-review/bulk-restore",
        { rule, days: 30 },
      );
      setMsg(
          ar.restoreSummary
            .replace("{rule}", rule)
            .replace("{n}", String(r.restored))
            .replace("{scanned}", String(r.scanned)),
        );
      void load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : ar.restoreFailed);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="nexus-detail space-y-3">
      <h2 className="text-base font-bold text-ink">
        {t("title")}
      </h2>
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-2 text-sm">{msg}</p>
      )}
      {stats && (
        <div className="grid gap-3 md:grid-cols-4">
          <div className={CARD}>
            <p className="text-sub text-xs">{t("shadowAgree")}</p>
            <p className="text-lg font-bold">
              {stats.shadow.agree_pct}%{" "}
              <span className="text-sub text-xs">
                ({stats.shadow.agree}/{stats.shadow.samples})
              </span>
            </p>
            <p className="text-sub text-xs">
              {stats.shadow.ready
                ? t("ready")
                : ar.notReadyTpl
                    .replace("{min}", String(stats.shadow.min_samples))
                    .replace(
                      "{agree}",
                      String(stats.shadow.min_agree_pct),
                    )}
            </p>
          </div>
          <div className={CARD}>
            <p className="text-sub text-xs">{t("falseKill")}</p>
            <p className="text-lg font-bold">{stats.shadow.false_deny}</p>
          </div>
          <div className={CARD}>
            <p className="text-sub text-xs">{t("falsePass")}</p>
            <p className="text-lg font-bold">{stats.shadow.false_pass}</p>
          </div>
          <div className={CARD}>
            <p className="text-sub text-xs">{t("today")}</p>
            <p className="text-sm">
              通过 {stats.today.pass} / 拒绝 {stats.today.deny} / 暂缓{" "}
              {stats.today.postpone}；待人工 {stats.pending_human}
            </p>
          </div>
        </div>
      )}
      <div className="flex flex-wrap items-center gap-2 text-sm">
        <input
          className="w-56 rounded border border-line px-2 py-1"
          placeholder={t("ruleFilter")}
          value={rule}
          onChange={(e) => setRule(e.target.value)}
        />
        <button
          className="rounded border border-line px-3 py-1"
          onClick={() => void load()}
        >
          {t("refresh")}
        </button>
        <button
          className="rounded border border-line px-3 py-1 disabled:opacity-50"
          disabled={busy}
          onClick={() => void replay()}
        >
          {t("replay")}
        </button>
        <button
          className="rounded border border-line px-3 py-1 disabled:opacity-50"
          disabled={busy}
          onClick={() => void restore()}
        >
          {t("restore")}
        </button>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead className="bg-soft text-left text-xs">
            <tr>
              <th className="p-2">{t("thTime")}</th>
              <th className="p-2">{t("thTorrent")}</th>
              <th className="p-2">{t("thVerdict")}</th>
              <th className="p-2">{t("thScore")}</th>
              <th className="p-2">{t("thHits")}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.id} className="border-t border-line align-top">
                <td className="p-2 text-xs text-sub">
                  {new Date(r.created_at).toLocaleString()}
                </td>
                <td className="p-2">
                  <a
                    className="text-link hover:underline"
                    href={`/torrents/${r.torrent_id}`}
                  >
                    {r.name}
                  </a>
                </td>
                <td className="p-2">{VERDICT[r.verdict] ?? r.verdict}</td>
                <td className="p-2">{r.score ?? "-"}</td>
                <td className="p-2 text-xs">
                  {(r.hits ?? []).map((h, i) => (
                    <p key={i}>
                      <span className="font-mono">{h.rule}</span>（{h.action}）
                      ：{h.detail}
                    </p>
                  ))}
                </td>
              </tr>
            ))}
            {rows.length === 0 && (
              <tr>
                <td className="p-4 text-center text-sub" colSpan={5}>
                  {t("empty")}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      {stats && stats.rule_health.length > 0 && (
        <div className={CARD}>
          <p className="mb-1 font-bold">{t("health")}</p>
          {stats.rule_health.map(([r, o, w]) => (
            <p key={r} className="text-xs">
              {r}：人工推翻 {o} 次 / 申诉翻案 {w} 次
              {o + 2 * w >= 10 ? ar.degradeLine : ""}
            </p>
          ))}
        </div>
      )}
    </section>
  );
}

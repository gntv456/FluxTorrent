"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** cheat_events 行（/admin/cheat-events，2026-10-07 保种组审计处置待办） */
export interface CheatEventRow {
  id: number;
  user_id: number;
  username: string | null;
  agent: string;
  peer_ip: string | null;
  reason: string;
  hits: number;
  first_seen: string;
  last_seen: string;
  resolved_at: string | null;
}

/** agent 键 → 短类型标签（ghost:/speed:/reset:/torrent: 前缀约定见 worker） */
function kindLabel(agent: string): string {
  if (agent.startsWith("ghost:")) return "ghost";
  if (agent.startsWith("speed:")) return "speed";
  if (agent.startsWith("reset:")) return "reset";
  if (agent.startsWith("torrent:")) return "gap";
  if (agent === "login_ip_spread") return "ip";
  return agent.slice(0, 12);
}

/**
 * 作弊事件待办（0367 起兼任「开案入口」）：未处置事件默认展示，
 * 可切换看已处置历史；每行可标记处置，或对该用户开立调查案件。
 * 从 cheater-box.tsx 拆出（300 行门禁）。
 */
export function CheatEventsTable() {
  const { dict, locale } = useI18n();
  const t = dict.cheaterbox;
  const [events, setEvents] = useState<CheatEventRow[] | null>(null);
  const [showDone, setShowDone] = useState(false);
  const [acting, setActing] = useState<number | null>(null);

  const loadEvents = (done: boolean) => {
    setEvents(null);
    const status = done ? "done" : "pending";
    api
      .get<{ items: CheatEventRow[] }>(
        `/api/v1/admin/cheat-events?status=${status}&limit=50`,
      )
      .then((r) => setEvents(r.items))
      .catch(() => setEvents([]));
  };

  useEffect(() => {
    loadEvents(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const resolveEvent = (id: number, resolved: boolean) => {
    setActing(id);
    api
      .post("/api/v1/admin/cheat-events/resolve", { id, resolved })
      .then(() => loadEvents(showDone))
      .catch(() => setActing(null))
      .finally(() => setActing(null));
  };

  /** 开案（0367 M1）：duplicate = 该用户已有进行中案件，静默刷新即可 */
  const openCase = (id: number) => {
    setActing(id);
    api
      .post("/api/v1/admin/cheat-cases/open", { event_id: id })
      .then(() => loadEvents(showDone))
      .catch(() => setActing(null))
      .finally(() => setActing(null));
  };

  const th = "px-3 py-2 text-left text-xs font-bold whitespace-nowrap";
  const td = "px-3 py-2 text-sm align-middle";

  return (
    <>
      {/* 作弊事件待办（处置后自动恢复该用户的做种收益结算） */}
      <div className="mt-2 mb-1 flex items-center justify-between">
        <h3 className="text-sm font-bold text-ink">{t.eventsTitle}</h3>
        <button
          type="button"
          className="text-xs text-[var(--baozi-orange-dark)] hover:underline"
          onClick={() => {
            const next = !showDone;
            setShowDone(next);
            loadEvents(next);
          }}
        >
          {showDone ? t.viewPending : t.viewDone}
        </button>
      </div>
      <p className="mb-2 text-xs text-sub">{t.eventsNote}</p>
      {events === null ? (
        <p className="funbox__empty">…</p>
      ) : events.length === 0 ? (
        <p className="funbox__empty">{t.eventsEmpty}</p>
      ) : (
        <div className="mb-4 overflow-x-auto">
          <table className="w-full border-collapse">
            <thead>
              <tr className="border-b border-[var(--border-soft)]">
                <th className={th}>{t.colEventUser}</th>
                <th className={th}>{t.colEventKind}</th>
                <th className={th}>{t.colEventHits}</th>
                <th className={th}>{t.colEventReason}</th>
                <th className={th}>{t.colEventTime}</th>
                <th className={th} aria-hidden="true" />
              </tr>
            </thead>
            <tbody>
              {events.map((e) => (
                <tr
                  key={e.id}
                  className={
                    "border-b border-dashed " +
                    "border-[var(--border-soft)]"
                  }
                >
                  <td className={td}>
                    <a
                      className={
                        "font-bold text-[var(--baozi-orange-dark)] " +
                        "hover:underline"
                      }
                      href={`/users/${e.user_id}`}
                    >
                      {e.username ?? `#${e.user_id}`}
                    </a>
                  </td>
                  <td className={td}>
                    <code className="rounded bg-[var(--bg-soft)] px-1 text-xs">
                      {kindLabel(e.agent)}
                    </code>
                  </td>
                  <td className={td}>{e.hits}</td>
                  <td className={`${td} max-w-[320px] truncate`}>
                    {e.reason}
                  </td>
                  <td
                    className={
                      `${td} whitespace-nowrap text-xs ` +
                      "text-[var(--text-faint)]"
                    }
                  >
                    {new Date(e.last_seen).toLocaleString(locale)}
                  </td>
                  <td className={`${td} whitespace-nowrap`}>
                    {e.resolved_at ? (
                      <button
                        type="button"
                        className="text-xs text-sub hover:underline"
                        disabled={acting === e.id}
                        onClick={() => resolveEvent(e.id, false)}
                      >
                        {acting === e.id ? t.resolving : t.viewPending}
                      </button>
                    ) : (
                      <>
                        <button
                          type="button"
                          className={
                            "text-xs font-bold " +
                            "text-[var(--baozi-orange-dark)] " +
                            "hover:underline"
                          }
                          disabled={acting === e.id}
                          onClick={() => resolveEvent(e.id, true)}
                        >
                          {acting === e.id ? t.resolving : t.resolve}
                        </button>
                        <button
                          type="button"
                          className={
                            "ml-2 text-xs font-bold " +
                            "text-[var(--baozi-orange-dark)] " +
                            "hover:underline"
                          }
                          disabled={acting === e.id}
                          onClick={() => openCase(e.id)}
                        >
                          {acting === e.id ? t.opening : t.openCase}
                        </button>
                      </>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}

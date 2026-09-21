"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { ToolTab } from "@/components/staff-tools";

/** 系统观测面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  数据库状态（dbstats）/ 系统日志（syslog）/ 位置管理（locations）/
 *  客户端黑白名单（agentrules）。 */

interface PgConn {
  state: string;
  count: number;
}
interface TableSize {
  relname: string;
  total_size: number;
  row_estimates: number;
}
interface DbStats {
  engine: string;
  database: string;
  connections: PgConn[];
  total_connections: number;
  database_size: number;
  slow_transactions: number;
  dead_tuples: number;
  tables: TableSize[];
}
interface SysLogItem {
  id: number;
  actor: string | null;
  action: string;
  ref_json: unknown;
  ip: string | null;
  created_at: string;
}
interface SysLogPage {
  items: SysLogItem[];
  total: number;
  page: number;
  per_page: number;
  pages: number;
}
interface LocationItem {
  net: string;
  netmask: number;
  logins: number;
  users: number;
  failed: number;
  last_seen: string | null;
}
interface LocationPage {
  items: LocationItem[];
  total: number;
  page: number;
  per_page: number;
  pages: number;
}

export function StaffSysPanel({
  tab,
  flash,
}: {
  tab: ToolTab;
  flash: (m: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  const [dbStats, setDbStats] = useState<DbStats | null>(null);
  const [logPage, setLogPage] = useState(1);
  const [logQ, setLogQ] = useState("");
  const [logData, setLogData] = useState<SysLogPage | null>(null);
  const [locPage, setLocPage] = useState(1);
  const [locData, setLocData] = useState<LocationPage | null>(null);
  const [agentRules, setAgentRules] = useState<
    | {
        id: number;
        mode: string;
        pattern: string;
        note: string | null;
        created_by: string | null;
        created_at: string;
      }[]
    | null
  >(null);
  const [arMode, setArMode] = useState("deny");
  const [arPattern, setArPattern] = useState("");
  const [arNote, setArNote] = useState("");
  const [busy, setBusy] = useState(false);

  // 系统日志/位置管理：按需分页加载
  useEffect(() => {
    api
      .get<DbStats | null>("/api/v1/admin/dbstats")
      .then(setDbStats)
      .catch(() => setDbStats(null));
  }, []);
  useEffect(() => {
    api
      .get<SysLogPage>(
        `/api/v1/admin/syslog?page=${logPage}&q=${encodeURIComponent(logQ)}`,
      )
      .then(setLogData)
      .catch(() => setLogData(null));
  }, [logPage, logQ]);
  useEffect(() => {
    api
      .get<LocationPage>(`/api/v1/admin/locations?page=${locPage}`)
      .then(setLocData)
      .catch(() => setLocData(null));
  }, [locPage]);

  async function guard(fn: () => Promise<void>, ok: string) {
    setBusy(true);
    try {
      await fn();
      flash(ok);
      setAgentRules(await api.get("/api/v1/admin/agentrules"));
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      {/* 数据库状态（mysql_stats → PostgreSQL） */}
      {tab === "dbstats" && dbStats && (
        <>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
            <div className="baozi-panel p-4">
              <p className="text-xs text-sub">{t.dsEngine}</p>
              <p className="text-base font-bold text-ink">
                {dbStats.engine} · {dbStats.database}
              </p>
            </div>
            <div className="baozi-panel p-4">
              <p className="text-xs text-sub">{t.dsConns}</p>
              <p className="num text-2xl font-bold text-ink">
                {dbStats.total_connections}
              </p>
            </div>
            <div className="baozi-panel p-4">
              <p className="text-xs text-sub">{t.dsSize}</p>
              <p className="num text-lg font-bold text-ink">
                {(dbStats.database_size / 1024 ** 2).toFixed(1)} MB
              </p>
            </div>
            <div className="baozi-panel p-4">
              <p className="text-xs text-sub">{t.dsSlow}</p>
              <p
                className={`num text-2xl font-bold ${dbStats.slow_transactions > 0 ? "text-danger" : "text-success"}`}
              >
                {dbStats.slow_transactions}
              </p>
            </div>
          </div>
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">{t.dsState}</td>
                <td className="colhead">{t.dsCount}</td>
              </tr>
              {dbStats.connections.map((c) => (
                <tr key={c.state}>
                  <td className="font-mono text-xs">{c.state}</td>
                  <td className="num">{c.count}</td>
                </tr>
              ))}
              {dbStats.connections.length === 0 && (
                <tr>
                  <td colSpan={2} className="py-6 text-center text-sub">
                    {t.dsEmpty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">{t.dsTable}</td>
                <td className="colhead">{t.dsRows}</td>
                <td className="colhead text-right">{t.dsTableSize}</td>
              </tr>
              {dbStats.tables.map((tb) => (
                <tr key={tb.relname}>
                  <td className="font-mono text-xs">{tb.relname}</td>
                  <td className="num">{tb.row_estimates}</td>
                  <td className="num text-right">
                    {(tb.total_size / 1024 ** 2).toFixed(2)} MB
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="text-xs text-sub">
            {t.dsDead.replace(
              "{n}",
              dbStats.dead_tuples.toLocaleString("zh-CN"),
            )}
          </p>
        </>
      )}

      {/* 系统日志（bitbucketlog → 审计日志） */}
      {tab === "syslog" && (
        <>
          <section className="baozi-panel p-4">
            <div className="cmgmt-form">
              <label>
                {t.slSearch}
                <input
                  value={logQ}
                  onChange={(e) => {
                    setLogQ(e.target.value);
                    setLogPage(1);
                  }}
                  placeholder="massmail / warn_user / freeleech"
                />
              </label>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">ID</td>
                <td className="colhead">{t.slActor}</td>
                <td className="colhead">{t.slAction}</td>
                <td className="colhead">{t.fldIp}</td>
                <td className="colhead">{t.mailAt}</td>
              </tr>
              {logData?.items.map((r) => (
                <tr key={r.id}>
                  <td className="num">{r.id}</td>
                  <td>{r.actor ?? "system"}</td>
                  <td className="font-mono text-xs">{r.action}</td>
                  <td className="font-mono text-xs">{r.ip ?? "—"}</td>
                  <td className="text-xs text-sub">
                    {new Date(r.created_at).toLocaleString("zh-CN")}
                  </td>
                </tr>
              ))}
              {(!logData || logData.items.length === 0) && (
                <tr>
                  <td colSpan={5} className="py-6 text-center text-sub">
                    {t.slEmpty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
          {logData && logData.pages > 1 && (
            <div className="flex items-center justify-between">
              <button
                className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-40"
                disabled={logPage <= 1}
                onClick={() => setLogPage((p) => p - 1)}
              >
                {dict.common.nextPage}
              </button>
              <span className="text-xs text-sub">
                {logData.page} / {logData.pages}（{logData.total}）
              </span>
              <button
                className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-40"
                disabled={logPage >= logData.pages}
                onClick={() => setLogPage((p) => p + 1)}
              >
                {dict.common.nextPage}
              </button>
            </div>
          )}
        </>
      )}

      {/* 位置管理（location → IP 网段视图） */}
      {tab === "locations" && (
        <>
          <p className="text-xs text-sub">{t.loNote}</p>
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">{t.loNet}</td>
                <td className="colhead">{t.loLogins}</td>
                <td className="colhead">{t.ipAccounts}</td>
                <td className="colhead">{t.loFailed}</td>
                <td className="colhead">{t.ipLastSeen}</td>
              </tr>
              {locData?.items.map((r) => (
                <tr key={`${r.net}/${r.netmask}`}>
                  <td className="font-mono">
                    {r.net}/{r.netmask}
                  </td>
                  <td className="num">{r.logins}</td>
                  <td className="num">{r.users}</td>
                  <td className="num">
                    {r.failed > 0 ? (
                      <span className="text-danger">{r.failed}</span>
                    ) : (
                      0
                    )}
                  </td>
                  <td className="text-xs text-sub">
                    {r.last_seen
                      ? new Date(r.last_seen).toLocaleString("zh-CN")
                      : "—"}
                  </td>
                </tr>
              ))}
              {(!locData || locData.items.length === 0) && (
                <tr>
                  <td colSpan={5} className="py-6 text-center text-sub">
                    {t.loEmpty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
          {locData && locData.pages > 1 && (
            <div className="flex items-center justify-between">
              <button
                className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-40"
                disabled={locPage <= 1}
                onClick={() => setLocPage((p) => p - 1)}
              >
                ‹
              </button>
              <span className="text-xs text-sub">
                {locData.page} / {locData.pages}（{locData.total}）
              </span>
              <button
                className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-40"
                disabled={locPage >= locData.pages}
                onClick={() => setLocPage((p) => p + 1)}
              >
                ›
              </button>
            </div>
          )}
        </>
      )}

      {/* 客户端黑白名单（G-06） */}
      {tab === "agentrules" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">
            {dict.agentRules2?.title ?? "客户端黑白名单"}
          </h2>
          <div className="cmgmt-form">
            <label>
              {dict.agentRules2?.mode ?? "类型"}
              <select
                value={arMode}
                onChange={(e) => setArMode(e.target.value)}
              >
                <option value="deny">
                  {dict.agentRules2?.deny ?? "黑名单"}
                </option>
                <option value="allow">
                  {dict.agentRules2?.allow ?? "白名单"}
                </option>
              </select>
            </label>
            <label>
              {dict.agentRules2?.pattern ?? "匹配串"}
              <input
                value={arPattern}
                onChange={(e) => setArPattern(e.target.value)}
                placeholder="Transmission/3"
              />
            </label>
            <label>
              {dict.agentRules2?.note ?? "备注"}
              <input
                value={arNote}
                onChange={(e) => setArNote(e.target.value)}
              />
            </label>
            <button
              className="baozi-button self-start"
              disabled={busy || !arPattern.trim()}
              onClick={() =>
                guard(async () => {
                  await api.post("/api/v1/admin/agentrules", {
                    mode: arMode,
                    pattern: arPattern.trim(),
                    note: arNote.trim() || null,
                  });
                  setArPattern("");
                  setArNote("");
                }, dict.agentRules2?.add ?? "已添加")
              }
            >
              {dict.agentRules2?.add ?? "添加规则"}
            </button>
            <p className="text-xs text-sub">
              {arMode === "deny"
                ? (dict.agentRules2?.modeDenyNote ?? "")
                : (dict.agentRules2?.modeAllowNote ?? "")}
            </p>
          </div>
          {agentRules === null ? (
            <button
              className="baozi-button mt-3"
              onClick={async () => {
                try {
                  setAgentRules(await api.get("/api/v1/admin/agentrules"));
                } catch {
                  setAgentRules([]);
                }
              }}
            >
              Load
            </button>
          ) : (
            <table className="nexus-table mt-3 text-xs">
              <thead>
                <tr>
                  <td className="colhead">
                    {dict.agentRules2?.mode ?? "类型"}
                  </td>
                  <td className="colhead">
                    {dict.agentRules2?.pattern ?? "匹配串"}
                  </td>
                  <td className="colhead">
                    {dict.agentRules2?.note ?? "备注"}
                  </td>
                  <td className="colhead" />
                </tr>
              </thead>
              <tbody>
                {agentRules.map((r) => (
                  <tr key={r.id}>
                    <td>
                      <span
                        className={`fun-status ${r.mode === "deny" ? "fun-status--banned" : "fun-status--normal"}`}
                      >
                        {r.mode === "deny"
                          ? (dict.agentRules2?.deny ?? "黑")
                          : (dict.agentRules2?.allow ?? "白")}
                      </span>
                    </td>
                    <td>
                      <code>{r.pattern}</code>
                    </td>
                    <td className="text-sub">{r.note ?? "—"}</td>
                    <td>
                      <button
                        className="min-h-[28px] rounded-full border border-line px-3 font-bold text-danger"
                        onClick={() =>
                          guard(async () => {
                            await api.post("/api/v1/admin/agentrules/delete", {
                              id: r.id,
                            });
                          }, "OK")
                        }
                      >
                        {dict.agentRules2?.del ?? "删除"}
                      </button>
                    </td>
                  </tr>
                ))}
                {agentRules.length === 0 && (
                  <tr>
                    <td colSpan={4} className="py-4 text-center text-sub">
                      {dict.agentRules2?.empty ?? "暂无规则"}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          )}
        </section>
      )}
    </>
  );
}

"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import type { ToolTab } from "@/components/staff-tools";
import { AgentRulesPanel } from "./staff-tools-sys-agentrules";
import { RuntimeLogPanel } from "./staff-tools-runtime";
import type {
  AgentRule,
  DbStats,
  LocationPage,
} from "./staff-tools-sys-shared";
import { PAGE_BTN_CLS } from "./staff-tools-sys-shared";

/** 系统观测面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  数据库状态（dbstats）/ 运行日志（syslog，见 staff-tools-runtime.tsx）/
 *  位置管理（locations）/ 客户端黑白名单（agentrules）。
 *  黑白名单拆至 ./staff-tools-sys-agentrules.tsx；类型拆至
 *  ./staff-tools-sys-shared.ts。 */

/** 慢事务计数配色 */
function slowCls(n: number) {
  return `num text-2xl font-bold ${n > 0 ? "text-danger" : "text-success"}`;
}

export function StaffSysPanel({
  tab,
  flash,
}: {
  tab: ToolTab;
  flash: (m: string) => void;
}) {
  const { dict, locale } = useI18n();
  const t = dict.stafftools;
  const [dbStats, setDbStats] = useState<DbStats | null>(null);
  const [locPage, setLocPage] = useState(1);
  const [locData, setLocData] = useState<LocationPage | null>(null);
  const [agentRules, setAgentRules] = useState<AgentRule[] | null>(null);

  // 数据库状态常显；位置管理按需分页加载
  useEffect(() => {
    api
      .get<DbStats | null>("/api/v1/admin/dbstats")
      .then(setDbStats)
      .catch(() => setDbStats(null));
  }, []);
  useEffect(() => {
    api
      .get<LocationPage>(`/api/v1/admin/locations?page=${locPage}`)
      .then(setLocData)
      .catch(() => setLocData(null));
  }, [locPage]);

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
              <p className={slowCls(dbStats.slow_transactions)}>
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
              dbStats.dead_tuples.toLocaleString(dateLocale(locale)),
            )}
          </p>
        </>
      )}

      {/* 运行日志（0218 G6：api/worker WARN+ 出口） */}
      {tab === "syslog" && <RuntimeLogPanel />}

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
                      ? new Date(r.last_seen).toLocaleString(dateLocale(locale))
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
                className={PAGE_BTN_CLS}
                disabled={locPage <= 1}
                onClick={() => setLocPage((p) => p - 1)}
              >
                ‹
              </button>
              <span className="text-xs text-sub">
                {locData.page} / {locData.pages}（{locData.total}）
              </span>
              <button
                className={PAGE_BTN_CLS}
                disabled={locPage >= locData.pages}
                onClick={() => setLocPage((p) => p + 1)}
              >
                ›
              </button>
            </div>
          )}
        </>
      )}

      {/* 客户端黑白名单（拆至 ./staff-tools-sys-agentrules.tsx） */}
      {tab === "agentrules" && (
        <AgentRulesPanel
          agentRules={agentRules}
          setAgentRules={setAgentRules}
          busy={false}
          flash={flash}
        />
      )}
    </>
  );
}

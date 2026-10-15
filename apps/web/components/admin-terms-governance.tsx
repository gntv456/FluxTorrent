"use client";

/**
 * 术语治理后台（0386 的前端面）。
 *
 * 这一屏解决的是术语表「变成没人敢动的坟」这件事：站长看得见
 * 有哪些键、哪些已废弃、覆盖率多少、改错了能回哪一版。
 *
 * 两个刻意呈现：
 *   1. **覆盖率旁边必须同时显示「退役引用数」**。只给一个百分比会
 *      制造假绿——覆盖率 90% 但引用着 3 个退役键，问题全被藏起来了。
 *   2. **巡检结果是「跑出来的」而不是「算出来的」**：点一下调后端真查
 *      一次库。前端不算覆盖率，因为前端的数会与后端漂移，而漂移的
 *      治理数字比没有数字更坏。
 */

import { useCallback, useEffect, useState } from "react";

import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";
import { StatePanel } from "@/components/ui/state-panel";
import { BTN_SM_BOLD, INPUT_MD } from "@/lib/ui-classes";

interface KeyRow {
  key: string;
  display_zh: string;
  category: string;
  status: "active" | "deprecated" | "retired";
  updated_at: string;
  retired_at?: string | null;
}

interface AuditResult {
  report_id: number;
  total_keys: number;
  active_keys: number;
  deprecated_keys: number;
  retired_keys: number;
  with_rule: number;
  coverage_pct: number;
  healthy: boolean;
  retired_refs: string[];
}

interface VersionRow {
  version: number;
  source: string;
  note: string;
  changed: number;
  created_at: string;
}

export function TermsGovernance() {
  const { dict } = useI18n();
  const [keys, setKeys] = useState<KeyRow[] | null>(null);
  const [versions, setVersions] = useState<VersionRow[]>([]);
  const [curVersion, setCurVersion] = useState<number | null>(null);
  const [audit, setAudit] = useState<AuditResult | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const [k, v] = await Promise.all([
        api.get<{ keys: KeyRow[] }>("/api/v1/admin/terms/keys"),
        api.get<{ versions: VersionRow[]; current_version: number }>(
          "/api/v1/admin/terms/versions",
        ),
      ]);
      setKeys(k.keys);
      setVersions(v.versions);
      setCurVersion(v.current_version);
      setErr(null);
    } catch (e) {
      setErr(apiErrorMessage(dict, e));
    }
  }, [dict]);

  useEffect(() => {
    void load();
  }, [load]);

  const runAudit = useCallback(async () => {
    setBusy(true);
    setMsg(null);
    try {
      setAudit(await api.post<AuditResult>("/api/v1/admin/terms/audit"));
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    } finally {
      setBusy(false);
    }
  }, [dict]);

  const setStatus = useCallback(
    async (key: string, status: string) => {
      setBusy(true);
      setMsg(null);
      try {
        await api.post("/api/v1/admin/terms/keys/status", {
          key,
          status,
        });
        await load();
      } catch (e) {
        setMsg(apiErrorMessage(dict, e));
      } finally {
        setBusy(false);
      }
    },
    [dict, load],
  );

  return (
    <StatePanel
      loading={keys === null && err === null}
      error={err}
      empty={keys !== null && keys.length === 0}
      onRetry={() => void load()}
    >
      {keys && (
        <div className="flex flex-col gap-4">
          <section className="flex flex-col gap-2">
            <div className="flex items-center justify-between gap-3">
              <h3 className="text-sm font-medium text-[var(--text-strong)]">
                覆盖率巡检
              </h3>
              <button
                type="button"
                className={BTN_SM_BOLD}
                disabled={busy}
                onClick={() => void runAudit()}
              >
                运行
              </button>
            </div>
            {audit ? (
              <div
                className="rounded-lg border border-[var(--border-soft)]
                           p-3 text-sm"
              >
                <div className="flex items-baseline gap-3">
                  <span className="text-lg text-[var(--text-strong)]">
                    {audit.coverage_pct.toFixed(1)}%
                  </span>
                  <span
                    className={
                      audit.healthy
                        ? "text-xs text-[var(--accent)]"
                        : "text-xs text-[var(--text-faint)]"
                    }
                  >
                    {audit.healthy ? "真绿" : "需处理"}
                  </span>
                </div>
                <p className="mt-1 text-xs text-[var(--text-muted)]">
                  在用 {audit.active_keys} · 有规则 {audit.with_rule}{" "}
                  · 退役引用 {audit.retired_refs.length}
                </p>
                {audit.retired_refs.length > 0 && (
                  <ul
                    className="mt-2 flex flex-col gap-1 text-xs
                               text-[var(--text-faint)]"
                  >
                    {audit.retired_refs.slice(0, 8).map((r) => (
                      <li key={r} className="break-all font-mono">
                        {r}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            ) : (
              <p className="text-xs text-[var(--text-faint)]">
                尚未巡检
              </p>
            )}
          </section>

          <section className="flex flex-col gap-2">
            <h3 className="text-sm font-medium text-[var(--text-strong)]">
              术语键（{keys.length}）
            </h3>
            <ul className="flex flex-col gap-2">
              {keys.map((k) => (
                <li
                  key={k.key}
                  className="flex items-center justify-between gap-3
                             rounded-lg border
                             border-[var(--border-soft)] p-2"
                >
                  <div className="min-w-0">
                    <p className="truncate font-mono text-sm
                                  text-[var(--text-strong)]">
                      {k.key}
                    </p>
                    <p className="text-xs text-[var(--text-faint)]">
                      {k.category} · {k.status}
                    </p>
                  </div>
                  <div className="flex shrink-0 gap-1">
                    {k.status === "active" && (
                      <button
                        type="button"
                        className={BTN_SM_BOLD}
                        disabled={busy}
                        onClick={() =>
                          void setStatus(k.key, "deprecated")
                        }
                      >
                        废弃
                      </button>
                    )}
                    {k.status === "deprecated" && (
                      <>
                        <button
                          type="button"
                          className={BTN_SM_BOLD}
                          disabled={busy}
                          onClick={() => void setStatus(k.key, "active")}
                        >
                          恢复
                        </button>
                        <button
                          type="button"
                          className={BTN_SM_BOLD}
                          disabled={busy}
                          onClick={() =>
                            void setStatus(k.key, "retired")
                          }
                        >
                          退役
                        </button>
                      </>
                    )}
                    {k.status === "retired" && (
                      <button
                        type="button"
                        className={BTN_SM_BOLD}
                        disabled={busy}
                        onClick={() => void setStatus(k.key, "active")}
                      >
                        撤回
                      </button>
                    )}
                  </div>
                </li>
              ))}
            </ul>
          </section>

          {curVersion !== null && (
            <section className="flex flex-col gap-2">
              <h3 className="text-sm font-medium text-[var(--text-strong)]">
                版本（当前 v{curVersion}）
              </h3>
              <ul className="flex flex-col gap-1 text-xs
                             text-[var(--text-muted)]">
                {versions.slice(0, 5).map((v) => (
                  <li key={v.version}>
                    v{v.version} · {v.source} · 改动 {v.changed}
                  </li>
                ))}
              </ul>
            </section>
          )}

          {msg && (
            <p role="alert" className="text-xs text-[var(--text-faint)]">
              {msg}
            </p>
          )}
        </div>
      )}
    </StatePanel>
  );
}

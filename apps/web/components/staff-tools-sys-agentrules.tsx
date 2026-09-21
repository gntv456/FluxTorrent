"use client";

/**
 * 系统观测·客户端黑白名单子面板（从 components/staff-tools-sys.tsx
 * 按域拆出）：G-06 客户端规则（deny/allow）表单与列表。
 */

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { AgentRule } from "./staff-tools-sys-shared";

interface AgentRulesProps {
  agentRules: AgentRule[] | null;
  setAgentRules: (v: AgentRule[] | null) => void;
  busy: boolean;
  flash: (m: string) => void;
}

/** 黑白名单删除小按钮 */
const DEL_BTN_CLS =
  "min-h-[28px] rounded-full border border-line px-3 font-bold text-danger";

/** 黑白名单徽标配色 */
function modeCls(mode: string) {
  return `fun-status ${
    mode === "deny" ? "fun-status--banned" : "fun-status--normal"
  }`;
}

export function AgentRulesPanel({
  agentRules,
  setAgentRules,
  busy,
  flash,
}: AgentRulesProps) {
  const { dict } = useI18n();
  const [arMode, setArMode] = useState("deny");
  const [arPattern, setArPattern] = useState("");
  const [arNote, setArNote] = useState("");
  const [busyLocal, setBusyLocal] = useState(false);

  async function guard(fn: () => Promise<void>, ok: string) {
    setBusyLocal(true);
    try {
      await fn();
      flash(ok);
      setAgentRules(await api.get("/api/v1/admin/agentrules"));
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusyLocal(false);
    }
  }
  const busy2 = busy || busyLocal;

  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-3 text-base font-bold text-ink">
        {dict.agentRules2?.title ?? "客户端黑白名单"}
      </h2>
      <div className="cmgmt-form">
        <label>
          {dict.agentRules2?.mode ?? "类型"}
          <select value={arMode} onChange={(e) => setArMode(e.target.value)}>
            <option value="deny">{dict.agentRules2?.deny ?? "黑名单"}</option>
            <option value="allow">{dict.agentRules2?.allow ?? "白名单"}</option>
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
          <input value={arNote} onChange={(e) => setArNote(e.target.value)} />
        </label>
        <button
          className="baozi-button self-start"
          disabled={busy2 || !arPattern.trim()}
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
              <td className="colhead">{dict.agentRules2?.mode ?? "类型"}</td>
              <td className="colhead">
                {dict.agentRules2?.pattern ?? "匹配串"}
              </td>
              <td className="colhead">{dict.agentRules2?.note ?? "备注"}</td>
              <td className="colhead" />
            </tr>
          </thead>
          <tbody>
            {agentRules.map((r) => (
              <tr key={r.id}>
                <td>
                  <span className={modeCls(r.mode)}>
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
                    className={DEL_BTN_CLS}
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
  );
}

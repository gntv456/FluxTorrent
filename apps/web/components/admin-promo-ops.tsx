"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { BTN_SM_BOLD } from "@/lib/ui-classes";

/** 运营工作台（0503 运营台账 #1/#2/#5 一面板三件）：
 *  促销效果归因表 + 过审自动促销规则 CRUD + 竞赛 CRUD。
 *  三件此前要么只读、要么只能 SQL 打库（auto_promo_rules /
 *  contests 建赛/促销归因）——运营自主能力的收口面板。 */

interface PromoEffect {
  id: number;
  kind: string;
  scope: string;
  category_id: string | null;
  downloads: number;
  users: number;
}

interface AutoRule {
  id: number;
  name: string;
  name_regex: string;
  min_size: number;
  max_size: number;
  category_id: number | null;
  kind: string;
  hours: number;
  enabled: boolean;
}

interface Contest {
  id: number;
  title: string;
  descr: string | null;
  is_active: boolean;
  joins: number;
}

const KINDS = ["free", "x2", "x2free", "half", "x2half", "p30"];

export function AdminPromoOps() {
  const { dict } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  const [tab, setTab] = useState<"effect" | "rules" | "contests">("effect");
  const [effects, setEffects] = useState<PromoEffect[] | null>(null);
  const [rules, setRules] = useState<AutoRule[] | null>(null);
  const [contests, setContests] = useState<Contest[] | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // 新建表单共享态（rules / contests 各一份）
  const [ruleName, setRuleName] = useState("");
  const [ruleKind, setRuleKind] = useState("free");
  const [ruleHours, setRuleHours] = useState(72);
  const [ctTitle, setCtTitle] = useState("");
  const [ctDescr, setCtDescr] = useState("");

  const load = useCallback(async () => {
    try {
      const [e, r, c] = await Promise.all([
        api.get<PromoEffect[]>("/api/v1/admin/promo-effect"),
        api.get<AutoRule[]>("/api/v1/admin/auto-promo-rules"),
        api.get<Contest[]>("/api/v1/admin/contests"),
      ]);
      setEffects(e ?? []);
      setRules(r ?? []);
      setContests(c ?? []);
    } catch {
      setEffects([]); setRules([]); setContests([]);
      setMsg(dict.common.loadFailed);
    }
  }, [dict]);

  useEffect(() => {
    void load();
  }, [load]);

  async function act(fn: () => Promise<unknown>, okMsg: string) {
    setBusy(true);
    try {
      await fn();
      setMsg(okMsg);
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : a.actionFailed);
    } finally {
      setBusy(false);
    }
  }

  const tabs = [
    ["effect", "促销效果归因"],
    ["rules", "过审自动促销规则"],
    ["contests", "竞赛管理"],
  ] as const;

  return (
    <section className="space-y-4">
      <div className="flex gap-2">
        {tabs.map(([k, label]) => (
          <button
            key={k}
            type="button"
            onClick={() => setTab(k)}
            className={`rounded px-3 py-2 text-sm ${
              tab === k ? "ui-btn ui-btn--primary" : "ui-btn ui-btn--secondary"
            }`}
          >
            {label}
          </button>
        ))}
      </div>
      {msg && <p className="text-sm text-sub">{msg}</p>}

      {tab === "effect" && (
        <div className="brand-panel p-4">
          <h2 className="mb-3 font-display text-lg">
            促销效果归因（近 10 场 · 窗口内完成下载）
          </h2>
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-line text-left text-xs text-sub">
                <th className="py-2">促销</th>
                <th className="py-2">范围</th>
                <th className="py-2">完成下载</th>
                <th className="py-2">独立用户</th>
              </tr>
            </thead>
            <tbody>
              {(effects ?? []).length === 0 && (
                <tr><td colSpan={4} className="py-6 text-center text-sub">暂无促销记录</td></tr>
              )}
              {(effects ?? []).map((e) => (
                <tr key={e.id} className="border-b border-line/50">
                  <td className="py-2 font-mono text-xs">#{e.id} {e.kind}</td>
                  <td className="py-2">{e.scope}</td>
                  <td className="py-2 font-mono">{e.downloads.toLocaleString()}</td>
                  <td className="py-2 font-mono">{e.users.toLocaleString()}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {tab === "rules" && (
        <div className="brand-panel p-4">
          <h2 className="mb-3 font-display text-lg">
            过审自动促销规则（命中第一条即自动挂）
          </h2>
          <div className="mb-4 flex flex-wrap items-end gap-2">
            <input
              placeholder="规则名"
              value={ruleName}
              onChange={(e) => setRuleName(e.target.value)}
              className="w-40 rounded border border-line px-2 py-2 text-sm"
            />
            <select
              value={ruleKind}
              onChange={(e) => setRuleKind(e.target.value)}
              className="rounded border border-line px-2 py-2 text-sm"
            >
              {KINDS.map((k) => (
                <option key={k} value={k}>{k}</option>
              ))}
            </select>
            <input
              type="number" min={1} max={2160}
              placeholder="时长(h)"
              value={ruleHours}
              onChange={(e) => setRuleHours(Number(e.target.value))}
              className="w-24 rounded border border-line px-2 py-2 text-sm"
            />
            <button
              type="button"
              disabled={busy || !ruleName.trim()}
              className={BTN_SM_BOLD}
              onClick={() =>
                act(
                  () =>
                    api.post("/api/v1/admin/auto-promo-rules", {
                      name: ruleName,
                      kind: ruleKind,
                      hours: ruleHours,
                    }),
                  "规则已创建",
                )
              }
            >
              新建规则
            </button>
          </div>
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-line text-left text-xs text-sub">
                <th className="py-2">规则</th>
                <th className="py-2">促销</th>
                <th className="py-2">时长</th>
                <th className="py-2">状态</th>
                <th className="py-2">操作</th>
              </tr>
            </thead>
            <tbody>
              {(rules ?? []).length === 0 && (
                <tr><td colSpan={5} className="py-6 text-center text-sub">暂无规则</td></tr>
              )}
              {(rules ?? []).map((r) => (
                <tr key={r.id} className="border-b border-line/50">
                  <td className="py-2">{r.name}</td>
                  <td className="py-2 font-mono text-xs">{r.kind}</td>
                  <td className="py-2">{r.hours}h</td>
                  <td className="py-2">{r.enabled ? "启用" : "停用"}</td>
                  <td className="py-2">
                    <button
                      type="button"
                      className="text-link text-xs"
                      disabled={busy}
                      onClick={() =>
                        act(
                          () =>
                            api.put(`/api/v1/admin/auto-promo-rules/${r.id}`, {
                              name: r.name, kind: r.kind, hours: r.hours,
                              enabled: !r.enabled,
                            }),
                          r.enabled ? "已停用" : "已启用",
                        )
                      }
                    >
                      {r.enabled ? "停用" : "启用"}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {tab === "contests" && (
        <div className="brand-panel p-4">
          <h2 className="mb-3 font-display text-lg">竞赛管理</h2>
          <div className="mb-4 flex flex-wrap items-end gap-2">
            <input
              placeholder="竞赛标题"
              value={ctTitle}
              onChange={(e) => setCtTitle(e.target.value)}
              className="w-56 rounded border border-line px-2 py-2 text-sm"
            />
            <input
              placeholder="赛制说明（可选）"
              value={ctDescr}
              onChange={(e) => setCtDescr(e.target.value)}
              className="w-64 rounded border border-line px-2 py-2 text-sm"
            />
            <button
              type="button"
              disabled={busy || !ctTitle.trim()}
              className={BTN_SM_BOLD}
              onClick={() =>
                act(
                  () =>
                    api.post("/api/v1/admin/contests", {
                      title: ctTitle,
                      descr: ctDescr || null,
                      prizes: [
                        { rank: 1, sparks: 5000 },
                        { rank: 2, sparks: 3000 },
                        { rank: 3, sparks: 1000 },
                      ],
                    }),
                  "竞赛已创建（默认 30 天、TOP3 奖励）",
                )
              }
            >
              建赛
            </button>
          </div>
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-line text-left text-xs text-sub">
                <th className="py-2">竞赛</th>
                <th className="py-2">报名</th>
                <th className="py-2">状态</th>
                <th className="py-2">操作</th>
              </tr>
            </thead>
            <tbody>
              {(contests ?? []).length === 0 && (
                <tr><td colSpan={4} className="py-6 text-center text-sub">暂无竞赛</td></tr>
              )}
              {(contests ?? []).map((c) => (
                <tr key={c.id} className="border-b border-line/50">
                  <td className="py-2">{c.title}</td>
                  <td className="py-2">{c.joins}</td>
                  <td className="py-2">{c.is_active ? "进行中" : "已结束"}</td>
                  <td className="py-2">
                    {c.is_active && (
                      <button
                        type="button"
                        className="text-link text-xs"
                        disabled={busy}
                        onClick={() =>
                          act(
                            () => api.del(`/api/v1/admin/contests/${c.id}`),
                            "已关闭（到期后自动结算）",
                          )
                        }
                      >
                        关闭
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

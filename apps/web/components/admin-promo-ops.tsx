"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { Badge } from "@/components/ui/badge";
import { BTN_SM_BOLD } from "@/lib/ui-classes";
import { PromoEffectTable } from "./admin-promo-ops-effect";
import { ContestsTab } from "./admin-promo-ops-contests";
import type { Contest } from "./admin-promo-ops-contests";

/** 运营工作台（0503 运营台账 #1/#2/#5 一面板三件）：
 *  促销效果归因表 + 过审自动促销规则 CRUD + 竞赛 CRUD。
 *  三件此前要么只读、要么只能 SQL 打库（auto_promo_rules /
 *  contests 建赛/促销归因）——运营自主能力的收口面板。 */

export interface PromoEffect {
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
  // 新建表单共享态（rules 一份；contests 态在 ContestsTab 内聚）
  const [ruleName, setRuleName] = useState("");
  const [ruleKind, setRuleKind] = useState("free");
  const [ruleHours, setRuleHours] = useState(72);

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
      {msg && <Badge tone="brand">{msg}</Badge>}

      {tab === "effect" && <PromoEffectTable rows={effects} />}
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
      )}      {tab === "contests" && <ContestsTab contests={contests} load={load} />}

    </section>
  );
}

"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { Badge } from "@/components/ui/badge";
import { BTN_SM_BOLD } from "@/lib/ui-classes";
import { OpsBoards } from "./admin-ops-boards";

/** 运营广播与生命周期漏斗（0505 运营体检批一面板两件）：
 *  ① 广播：PM 批量广播（复用 massmail 0497 圈选口径）+ 聊天室官方
 *     广播——两者此前分别只能搭批量发放便车/藏在促销 job。
 *  ② 漏斗：注册 cohort 四步转化（注册→首下→首做种→首签到），
 *     数据此前只有单人 onboarding_facts，无聚合面。 */

interface FunnelRow {
  week: string;
  registered: number;
  firstDownload: number;
  firstSeed: number;
  firstCheckin: number;
}

const AUDIENCES = [
  ["all", "全员"],
  ["active30", "30 天活跃"],
  ["dormant90", "90 天未活跃"],
  ["vip", "VIP/捐赠"],
  ["class:1", "等级≥1"],
  ["class:5", "等级≥5"],
  ["class:98", "等级≥98"],
] as const;

export function AdminOpsBroadcast() {
  const { dict } = useI18n();
  const [tab, setTab] = useState<"funnel" | "pm" | "shout" | "boards">(
    "funnel",
  );
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [audience, setAudience] = useState<string>("all");
  const [pmSubject, setPmSubject] = useState("");
  const [pmBody, setPmBody] = useState("");
  const [shoutMsg, setShoutMsg] = useState("");
  const [funnel, setFunnel] = useState<FunnelRow[] | null>(null);
  const [funnelWeeks, setFunnelWeeks] = useState(8);

  const loadFunnel = useCallback(async (weeks: number) => {
    try {
      const r = await api.get<{ rows: FunnelRow[] }>(
        `/api/v1/admin/funnel?weeks=${weeks}`,
      );
      setFunnel(r.rows ?? []);
    } catch {
      setFunnel([]);
      setMsg(dict.common.loadFailed);
    }
  }, [dict]);

  useEffect(() => {
    void loadFunnel(funnelWeeks);
  }, [loadFunnel, funnelWeeks]);

  async function send(fn: () => Promise<unknown>, okMsg: string) {
    setBusy(true);
    try {
      const r = await fn() as Record<string, unknown>;
      const detail =
        r.sent != null
          ? `：目标 ${r.targeted} 人，实际送达 ${r.sent}（偏好关闭 ${r.skipped_by_pref}）`
          : "";
      setMsg(okMsg + detail);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  const tabs = [
    ["funnel", "生命周期漏斗"],
    ["boards", "签到/H&R 看板"],
    ["pm", "站内信广播"],
    ["shout", "聊天室广播"],
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

      {tab === "funnel" && (
        <div className="brand-panel p-4">
          <div className="mb-3 flex items-center gap-2">
            <h2 className="font-display text-lg">注册 cohort 生命周期漏斗</h2>
            <select
              value={funnelWeeks}
              onChange={(e) => setFunnelWeeks(Number(e.target.value))}
              className="rounded border border-line px-2 py-1 text-sm"
              aria-label="回看周数"
            >
              {[4, 8, 12, 26].map((w) => (
                <option key={w} value={w}>近 {w} 周</option>
              ))}
            </select>
          </div>
          <div className="brand-wide-table-scroll">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-line text-left text-xs text-sub">
                  <th className="py-2">注册周</th>
                  <th className="py-2">注册</th>
                  <th className="py-2">首次下载</th>
                  <th className="py-2">首次做种</th>
                  <th className="py-2">首次签到</th>
                </tr>
              </thead>
              <tbody>
                {(funnel ?? []).length === 0 && (
                  <tr>
                    <td colSpan={5} className="py-6 text-center text-sub">
                      暂无数据
                    </td>
                  </tr>
                )}
                {(funnel ?? []).map((r) => {
                  const pct = (v: number) =>
                    r.registered > 0
                      ? `${Math.round((v / r.registered) * 100)}%`
                      : "";
                  return (
                  <tr key={r.week} className="border-b border-line">
                    <td className="py-2">{r.week.slice(0, 10)}</td>
                    <td className="py-2">{r.registered}</td>
                    <td className="py-2">
                      {r.firstDownload}
                      <span className="ml-1 text-xs text-sub">
                        {pct(r.firstDownload)}
                      </span>
                    </td>
                    <td className="py-2">
                      {r.firstSeed}
                      <span className="ml-1 text-xs text-sub">
                        {pct(r.firstSeed)}
                      </span>
                    </td>
                    <td className="py-2">
                      {r.firstCheckin}
                      <span className="ml-1 text-xs text-sub">
                        {pct(r.firstCheckin)}
                      </span>
                    </td>
                  </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {tab === "boards" && <OpsBoards />}

      {tab === "pm" && (
        <div className="brand-panel p-4">
          <h2 className="mb-3 font-display text-lg">
            站内信批量广播（收件人「系统通知」关闭者自动跳过）
          </h2>
          <div className="cmgmt-form">
            <label>
              圈选人群
              <select
                value={audience}
                onChange={(e) => setAudience(e.target.value)}
                className="rounded border border-line px-2 py-2 text-sm"
              >
                {AUDIENCES.map(([k, label]) => (
                  <option key={k} value={k}>{label}</option>
                ))}
              </select>
            </label>
            <label>
              主题
              <input
                value={pmSubject}
                onChange={(e) => setPmSubject(e.target.value)}
                className="rounded border border-line px-2 py-2 text-sm"
              />
            </label>
            <label>
              正文
              <textarea
                rows={5}
                value={pmBody}
                onChange={(e) => setPmBody(e.target.value)}
              />
            </label>
            <button
              type="button"
              disabled={busy || !pmSubject.trim() || !pmBody.trim()}
              className={BTN_SM_BOLD}
              onClick={() =>
                send(
                  () =>
                    api.post("/api/v1/admin/pm-broadcast", {
                      subject: pmSubject,
                      body: pmBody,
                      audience,
                    }),
                  "广播已发送",
                )
              }
            >
              发送广播
            </button>
          </div>
        </div>
      )}

      {tab === "shout" && (
        <div className="brand-panel p-4">
          <h2 className="mb-3 font-display text-lg">
            聊天室官方广播（以系统身份发一条喊话）
          </h2>
          <div className="cmgmt-form">
            <label>
              广播内容（1-300 字）
              <input
                value={shoutMsg}
                onChange={(e) => setShoutMsg(e.target.value)}
                className="rounded border border-line px-2 py-2 text-sm"
              />
            </label>
            <button
              type="button"
              disabled={busy || !shoutMsg.trim()}
              className={BTN_SM_BOLD}
              onClick={() =>
                send(
                  () =>
                    api.post("/api/v1/admin/shout-broadcast", {
                      message: shoutMsg,
                    }),
                  "广播已发布",
                )
              }
            >
              发布广播
            </button>
          </div>
        </div>
      )}
    </section>
  );
}

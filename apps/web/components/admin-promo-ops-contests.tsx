"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { BTN_SM_BOLD } from "@/lib/ui-classes";

/** 运营工作台·竞赛管理 tab（0506 从 admin-promo-ops.tsx 按域拆出守
 *  300 门禁）：建赛（含赛制选择）+ 列表 + 关闭。赛制 0506 参数化
 *  （uploads/downloads/seedtime，与 0506 迁移 CHECK 同源）。 */

export interface Contest {
  id: number;
  title: string;
  descr: string | null;
  is_active: boolean;
  joins: number;
  metric: string;
}

export const METRICS = [
  ["uploads", "上传竞速（过审种数）"],
  ["downloads", "下载竞速（完成下载数）"],
  ["seedtime", "做种耐力（做种时长）"],
] as const;

export function ContestsTab({
  contests,
  load,
}: {
  contests: Contest[] | null;
  load: () => Promise<void>;
}) {
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [ctTitle, setCtTitle] = useState("");
  const [ctDescr, setCtDescr] = useState("");
  const [ctMetric, setCtMetric] = useState("uploads");

  async function act(fn: () => Promise<unknown>, okMsg: string) {
    setBusy(true);
    try {
      await fn();
      setMsg(okMsg);
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="brand-panel p-4">
      <h2 className="mb-3 font-display text-lg">竞赛管理</h2>
      {msg && <p className="mb-2 text-sm text-sub">{msg}</p>}
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
        <select
          value={ctMetric}
          onChange={(e) => setCtMetric(e.target.value)}
          className="rounded border border-line px-2 py-2 text-sm"
          aria-label="计分赛制"
        >
          {METRICS.map(([k, label]) => (
            <option key={k} value={k}>{label}</option>
          ))}
        </select>
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
                  metric: ctMetric,
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
            <tr>
              <td colSpan={4} className="py-6 text-center text-sub">
                暂无竞赛
              </td>
            </tr>
          )}
          {(contests ?? []).map((c) => (
            <tr key={c.id} className="border-b border-line/50">
              <td className="py-2">
                {c.title}
                <span className="ml-2 text-xs text-sub">
                  {METRICS.find(([k]) => k === c.metric)?.[1] ?? c.metric}
                </span>
              </td>
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
  );
}

"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { BTN_SM_SKY } from "@/lib/ui-classes";

/** C4 新手运营模板后台页（0226）：GET/POST /admin/onboarding。
 *  查看当前模板态 + 参数簇现值；一键切换 strict（考核淘汰制）/
 *  lenient（缓冲宽进制）/ none（维持现状，参数保持手调值）。 */

interface OnboardingStatus {
  current: string;
  exam_auto_assign: boolean;
  params: Record<string, string>;
}

const PARAM_LABELS: Record<string, string> = {
  hr_hours: "H&R 考察时长（小时）",
  hr_violation_limit: "H&R 违规上限",
  hr_prewarn_hours: "H&R 到期预警提前（小时）",
  initial_upload_gb: "新用户初始上传量（GB）",
  exam_onboard_days: "考核派发窗口（天）",
};

export function AdminOnboarding() {
  const { dict } = useI18n();
  void dict;
  const [st, setSt] = useState<OnboardingStatus | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setSt(
        await api.get<OnboardingStatus>("/api/v1/admin/onboarding"),
      );
    } catch {
      /* 面板级静默 */
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load]);

  async function apply(preset: string) {
    setBusy(true);
    setMsg(null);
    try {
      const res = await api.post<{ applied: string[] }>(
        "/api/v1/admin/onboarding",
        { preset },
      );
      setMsg(`已应用（写入 ${res.applied.length} 项）`);
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  if (!st) return <p className="py-8 text-center text-sub">…</p>;

  const presets = [
    {
      code: "strict",
      name: "考核淘汰制",
      desc: "高压高留存（NexusPHP 式）：新人考核自动派发；H&R 96h / 上限 3；低保户可降级；无初始缓冲。",
    },
    {
      code: "lenient",
      name: "缓冲宽进制",
      desc: "宽进宽养（UNIT3D 式）：初始 50GB 缓冲；H&R 168h / 上限 5 / 提前 48h 预警；无考核、不自动降级。",
    },
    {
      code: "none",
      name: "维持现状",
      desc: "只记录选择态，参数保持当前手调值。",
    },
  ];

  return (
    <section className="space-y-4">
      <div className="baozi-panel p-4">
        <h2 className="mb-1 font-display text-lg">新手运营模板</h2>
        <p className="text-xs text-sub">
          一键写入一组新手期运营参数（考核 / H&R / 初始缓冲 / 降级）。
          切换只改配置，不动已有用户数据；之后可在「站点设定」逐项微调。
          当前：{st.current === "strict" ? "考核淘汰制" : st.current === "lenient" ? "缓冲宽进制" : "未选择"}
          （考核自动派发：{st.exam_auto_assign ? "开" : "关"}）
        </p>
        <div className="mt-3 grid gap-3 sm:grid-cols-3">
          {presets.map((p) => (
            <button
              key={p.code}
              disabled={busy}
              onClick={() => void apply(p.code)}
              className={`rounded-lg border p-3 text-left transition-colors hover:border-accent ${
                st.current === p.code ? "border-accent" : "border-line"
              }`}
            >
              <div className="text-sm font-medium">{p.name}</div>
              <div className="mt-1 text-xs text-sub">{p.desc}</div>
            </button>
          ))}
        </div>
        {msg && <p className="mt-3 text-sm">{msg}</p>}
      </div>

      <div className="baozi-panel overflow-x-auto p-4">
        <h3 className="mb-2 text-sm font-semibold">参数现值</h3>
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-line text-left text-xs text-sub">
              <th className="py-2">参数</th>
              <th className="py-2">当前值</th>
            </tr>
          </thead>
          <tbody>
            {Object.entries(PARAM_LABELS).map(([k, label]) => (
              <tr key={k} className="border-b border-line/50">
                <td className="py-2">{label}</td>
                <td className="py-2 font-mono">
                  {st.params[k] ?? "—"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

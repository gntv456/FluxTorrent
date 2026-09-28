"use client";

/**
 * G31 后台「抽卡运营」面板（方案 §5 G31-C 六 pane 的最小合体）：
 * 发券/发碎片表单（staff.rs 端点，幂等键 UI 侧自动生成——同参数重提
 * 会被 400「已发放过」拒绝，这是刻意的可审计语义）+ 池/卡/面板行只读总览。
 * 池参数与卡定义的 CRUD 走 SQL/内容包（机制进代码、内容进包，方案 §3），
 * 本面板只做发放与巡检，不复制编辑器。
 */
import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

interface GrantForm {
  userId: string;
  amount: string;
  note: string;
}

const EMPTY: GrantForm = { userId: "", amount: "100", note: "" };

export function AdminGacha() {
  const [tk, setTk] = useState<GrantForm>(EMPTY);
  const [sh, setSh] = useState<GrantForm>(EMPTY);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [stats, setStats] = useState<{
    banners: number;
    cards: number;
    draws: number;
    litUsers: number;
    tickets: number;
    shards: number;
  } | null>(null);

  const load = useCallback(async () => {
    try {
      setStats(await api.get("/api/v1/admin/gacha/stats"));
    } catch {
      setStats(null);
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load]);

  const submit = useCallback(
    async (
      kind: "grant" | "grant-shards",
      form: GrantForm,
    ) => {
      setMsg(null);
      try {
        const d = await api.post<{ [k: string]: number }>(
          `/api/v1/admin/gacha/${kind}`,
          {
            user_id: Number(form.userId),
            amount: Number(form.amount),
            note: form.note,
            idempotency_key:
              `ui-${kind}-${form.userId}-${Date.now()}`,
          },
        );
        const bal = d.ticketBalance ?? d.shardBalance;
        setMsg({
          ok: true,
          text: `已发放 ${d.granted}，余额 ${bal}`,
        });
        await load();
      } catch (e) {
        setMsg({
          ok: false,
          text: e instanceof ApiError ? e.message : String(e),
        });
      }
    },
    [load],
  );

  const field =
    "w-full rounded border border-border bg-surface px-2 py-1.5 text-sm";
  const form = (
    f: GrantForm,
    set: (f: GrantForm) => void,
    kind: "grant" | "grant-shards",
    title: string,
  ) => (
    <form
      className="flex flex-col gap-2"
      onSubmit={(e) => {
        e.preventDefault();
        void submit(kind, f);
      }}
    >
      <h3 className="text-sm font-semibold">{title}</h3>
      <input
        className={field}
        placeholder="用户 ID"
        value={f.userId}
        onChange={(e) => set({ ...f, userId: e.target.value })}
        required
        inputMode="numeric"
      />
      <input
        className={field}
        placeholder="数量（1~1,000,000）"
        value={f.amount}
        onChange={(e) => set({ ...f, amount: e.target.value })}
        required
        inputMode="numeric"
      />
      <input
        className={field}
        placeholder="备注（审计可见）"
        value={f.note}
        onChange={(e) => set({ ...f, note: e.target.value })}
        required
      />
      <button className="btn btn-primary text-sm" type="submit">
        发放
      </button>
    </form>
  );

  return (
    <section className="flex flex-col gap-4">
      {msg && (
        <div
          className={`card p-3 text-sm ${msg.ok ? "" : "text-destructive"}`}
        >
          {msg.text}
        </div>
      )}
      <div className="grid gap-4 sm:grid-cols-2">
        {form(tk, setTk, "grant", "发抽卡券")}
        {form(sh, setSh, "grant-shards", "发碎片")}
      </div>
      <div className="card flex flex-wrap gap-x-6 gap-y-1 p-4 text-sm">
        <span>卡池 {stats?.banners ?? "—"}</span>
        <span>卡定义 {stats?.cards ?? "—"}</span>
        <span>累计抽取 {stats?.draws ?? "—"}</span>
        <span>点亮用户 {stats?.litUsers ?? "—"}</span>
        <span>流通券 {stats?.tickets ?? "—"}</span>
        <span>流通碎片 {stats?.shards ?? "—"}</span>
      </div>
      <p className="text-xs text-muted">
        卡池参数/卡定义的修改走数据库或内容包（机制进代码、内容进包）；
        经济守卫在每次抽取请求上强制执行（返还率 &gt;100% 拒抽）。
      </p>
    </section>
  );
}

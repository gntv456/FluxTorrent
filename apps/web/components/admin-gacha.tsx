"use client";

/**
 * G31 后台「抽卡运营」面板（方案 §5 G31-C 六 pane 的最小合体）：
 * 发券/发碎片表单 + 池/卡/面板行只读总览。幂等键按「提交内容 + 当天」
 * 派生（`@/lib/idem-key`，0291），所以同一天里连点两次是同一批，
 * 服务端唯一约束会拒第二次——早先用 `Date.now()` 现造键时，这条承诺
 * 并不成立（每次点击都是新键，双击就是两笔真发放）。
 * 池参数与卡定义的 CRUD 走 SQL/内容包（机制进代码、内容进包，方案 §3），
 * 本面板只做发放与巡检，不复制编辑器。
 */
import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { idemKey } from "@/lib/idem-key";

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
  const [busy, setBusy] = useState(false);
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
      setBusy(true);
      // 幂等键按提交内容算（同一天同参数 = 同一批）。此前这里是
      // `ui-${kind}-${userId}-${Date.now()}`——每次点击都是新键，
      // 后端那道唯一约束永远不会命中，双击就是两笔真发放。
      const payload = {
        user_id: Number(form.userId),
        amount: Number(form.amount),
        note: form.note,
      };
      try {
        const d = await api.post<{ [k: string]: number }>(
          `/api/v1/admin/gacha/${kind}`,
          {
            ...payload,
            idempotency_key: idemKey(payload, "gacha"),
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
      } finally {
        setBusy(false);
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
      <button
        className="btn btn-primary text-sm"
        type="submit"
        disabled={busy}
      >
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

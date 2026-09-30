"use client";

/**
 * 奖池编辑器（配置面第一件）。
 *
 * 行数据一律从 `GET /games` 的 jgg 投影取 —— 那是玩法真正读的同一份表，
 * 面板不自建第二份清单；EV 也不在这里算，保存成功后由运营面板的 EV 表
 * （服务端 pool_ev）刷新。
 *
 * 关闸在写侧：POST 被 400 拒时，**服务端原因原样回显**，那就是给站长看的账
 * （「EV 1.200 >= 1：在增发」比任何前端提示都准）。
 */

import { useCallback, useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";
import {
  AdminArcadePoolRows,
  type CatalogItem,
  type PoolRowView,
} from "./admin-arcade-pool-rows";
import { api, ApiError } from "@/lib/api-client";

interface Row {
  label: string;
  weight_permille: number;
  payout: number;
  kind?: string;
  item_key?: string;
  qty?: number;
  /** 猜大小用：这一档在哪一区付 */
  side?: string;
}

interface CatItem {
  key: string;
  name: string;
  icon: string;
  anchor: number;
}

/** 可编辑的奖池清单：game 决定读 /games 的哪一份投影，pool_key/label 是行表上的键。
 *  加一张池 = 在这里加一行 + 服务端有对应的 game 投影，不在界面里写死中文。 */
const POOLS = [
  { game: "jgg", key: "jgg_default", labelKey: "labelJgg" },
  { game: "scratch", key: "scratch_default", labelKey: "labelScratch" },
  { game: "bigsmall", key: "bigsmall_default", labelKey: "labelBigsmall" },
] as const;

type PoolProj = { ticket: number; prizes: Row[] };

const CELL =
  "w-full rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--surface-card)] px-2 py-1 text-xs";

export function AdminArcadePool({
  onChanged,
}: {
  onChanged?: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.adminArcade.pool;
  const [ticket, setTicket] = useState(0);
  const [rows, setRows] = useState<Row[]>([]);
  const [catalog, setCatalog] = useState<CatItem[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [game, setGame] = useState<string>("jgg");
  const pool = POOLS.find((p) => p.game === game) ?? POOLS[0];

  const load = useCallback(async () => {
    try {
      // 读的就是玩法真正读的那份投影：面板与玩法之间没有第二份清单
      const g = await api.get<Record<string, PoolProj | undefined>>(
        "/api/v1/games",
      );
      const proj = g[pool.game];
      setTicket(proj?.ticket ?? 0);
      setRows(proj?.prizes ?? []);
      // 物品位的候选来自目录本身，面板不另写一份物品清单
      const ov = await api.get<{ items?: CatItem[] }>(
        "/api/v1/admin/arcade/overview",
      );
      setCatalog(ov.items ?? []);
      return true;
    } catch {
      setMsg(t.loadFail);
      return false;
    }
  }, [t.loadFail, pool.game]);

  useEffect(() => {
    void load();
  }, [load]);

  const set = (i: number, patch: Partial<Row>) =>
    setRows((rs) => rs.map((r, k) => (k === i ? { ...r, ...patch } : r)));

  async function save() {
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/admin/arcade/pool", {
        pool_key: pool.key,
        game: pool.game,
        label: t[pool.labelKey],
        ticket,
        entries: rows.map((r) => ({
          label: r.label,
          weight: r.weight_permille,
          enabled: true,
          kind: r.kind ?? "magic",
          ...(r.kind === "item"
            ? { item_key: r.item_key, qty: r.qty ?? 1 }
            : { payout: r.payout }),
          side: r.side ?? "any",
        })),
      });
      setMsg(t.saved);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.saveFail);
    }
    // 无论成没成都回读：被拒时让面板显示表里的真值，而不是表单里的猜测
    await load();
    onChanged?.();
    setBusy(false);
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-3">
        <h3 className="text-sm font-bold">{t.title}</h3>
        <div className="flex items-center gap-2 text-xs">
          <span className="text-sub">{t.pickGame}</span>
          <select
            className={`${CELL} w-28`}
            value={game}
            onChange={(e) => setGame(e.target.value)}
          >
            {POOLS.map((p) => (
              <option key={p.game} value={p.game}>
                {t[p.labelKey]}
              </option>
            ))}
          </select>
          <span className="text-sub">{t.ticket}</span>
          <input
            type="number"
            min={1}
            className={`${CELL} w-24 text-right`}
            value={ticket}
            onChange={(e) => setTicket(Number(e.target.value) || 0)}
          />
        </div>
      </div>

      <AdminArcadePoolRows
        rows={rows}
        catalog={catalog}
        showSide={pool.game === "bigsmall"}
        onSet={set}
      />

      <div className="flex items-center gap-3">
        <button
          type="button"
          className={
            "rounded-full bg-coral px-4 py-2 text-xs font-bold " +
            "text-white disabled:opacity-60"
          }
          disabled={busy || rows.length === 0}
          onClick={() => void save()}
        >
          {busy ? t.saving : t.save}
        </button>
        <button
          type="button"
          className="rounded-full border border-line px-3 py-2 text-xs"
          disabled={busy}
          onClick={() => {
            setRows((rs) => [
              ...rs,
              { label: t.newRow, weight_permille: 0, payout: 0 },
            ]);
          }}
        >
          {t.addRow}
        </button>
        <button
          type="button"
          className="rounded-full border border-line px-3 py-2 text-xs"
          disabled={busy || rows.length === 0}
          onClick={() => setRows((rs) => rs.slice(0, -1))}
        >
          {t.delRow}
        </button>
      </div>

      {msg && (
        <p
          className={`rounded-[var(--r-sm)] p-2 text-xs ${
            msg === t.saved ? "bg-mint-soft text-ink" : "bg-sun-soft text-ink"
          }`}
        >
          {msg}
        </p>
      )}
      <p className="text-[11px] text-sub">{t.note}</p>
    </div>
  );
}

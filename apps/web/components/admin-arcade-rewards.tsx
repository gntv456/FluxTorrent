"use client";

/**
 * 确定侧奖励编辑器（0247 的两张行表）。
 *
 * 周常与里程碑只差两个列名（game_ref/target 与 season_key/need），
 * 界面上合成一张表、用「侧」这一列说明它属于哪一侧 —— 两套控件各写一遍
 * 必然漂移，而服务端 upsert 也本来就是同一份 SQL 换列名。
 *
 * 这里能配的是「达到什么、给什么」；「给了之后进谁的账」由服务端定：
 * 魔力走 spark_ledger(kind='arcade')，物品走发放账(side='det')，
 * 两边都会被娱乐屋预算闸看见 —— 所以这一页没有「绕过预算」的开关。
 */

import { useCallback, useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";
import { api, ApiError } from "@/lib/api-client";

interface Row {
  kind: string;
  code: string;
  scope: string;
  target: number;
  reward_spark: number;
  item_key: string;
  item_qty: number;
  enabled: boolean;
  sort: number;
}

interface DefRow {
  code: string;
  target?: number;
  need?: number;
  reward: number;
  item_key?: string | null;
  item_qty?: number;
  enabled: boolean;
  ref?: string;
  season_key?: string;
}

const CELL =
  "w-full rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--surface-card)] px-1.5 py-1 text-xs";
const BTN =
  "rounded-full border border-line px-3 py-1 text-[11px] " +
  "font-bold disabled:opacity-50";

export function AdminArcadeRewards({
  onChanged,
}: {
  onChanged?: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.adminArcade.rewards;
  const [rows, setRows] = useState<Row[]>([]);
  const [items, setItems] = useState<{ key: string; name: string }[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const d = await api.get<{
        items?: { key: string; name: string }[];
        defs?: {
          season_key: string;
          quests?: DefRow[];
          milestones?: DefRow[];
        };
      }>("/api/v1/admin/arcade/overview");
      const q = (d.defs?.quests ?? []).map((r, i) => ({
        kind: "quest",
        code: r.code,
        scope: r.ref ?? "*",
        target: r.target ?? 0,
        reward_spark: r.reward,
        item_key: r.item_key ?? "",
        item_qty: r.item_qty ?? 1,
        enabled: r.enabled,
        sort: (i + 1) * 10,
      }));
      const m = (d.defs?.milestones ?? []).map((r, i) => ({
        kind: "milestone",
        code: r.code,
        scope: r.season_key ?? d.defs?.season_key ?? "S1",
        target: r.need ?? 0,
        reward_spark: r.reward,
        item_key: r.item_key ?? "",
        item_qty: r.item_qty ?? 1,
        enabled: r.enabled,
        sort: (i + 1) * 10,
      }));
      setRows([...q, ...m]);
      setItems(d.items ?? []);
    } catch {
      setMsg(t.saveFail);
    }
  }, [t.saveFail]);

  useEffect(() => {
    void load();
  }, [load]);

  const set = (i: number, patch: Partial<Row>) =>
    setRows(rows.map((r, j) => (j === i ? { ...r, ...patch } : r)));

  async function save(r: Row) {
    setBusy(r.code);
    setMsg(null);
    try {
      await api.post("/api/v1/admin/arcade/rewards", r);
      setMsg(t.saved);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : String(e));
    }
    await load();
    onChanged?.();
    setBusy(null);
  }

  return (
    <div className="flex flex-col gap-3">
      <h3 className="text-sm font-bold">{t.title}</h3>
      <div className="overflow-x-auto">
        <table className="nexus-table w-full text-xs">
          <thead>
            <tr className="text-left text-sub">
              <th className="py-1">{t.colKind}</th>
              <th className="py-1">{t.colCode}</th>
              <th className="py-1">{t.colScope}</th>
              <th className="py-1 text-right">{t.colTarget}</th>
              <th className="py-1 text-right">{t.colSpark}</th>
              <th className="py-1">{t.colItem}</th>
              <th className="py-1 text-right">{t.colQty}</th>
              <th className="py-1 text-center">{t.colOn}</th>
              <th className="py-1" />
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr key={r.kind + r.code} className="border-t border-line">
                <td className="py-1 pr-1 text-sub">
                  {r.kind === "quest" ? t.kindQuest : t.kindMilestone}
                </td>
                <td className="py-1 pr-1">{r.code}</td>
                <td className="py-1 pr-1">
                  <input
                    className={`${CELL} w-24`}
                    value={r.scope}
                    onChange={(e) => set(i, { scope: e.target.value })}
                  />
                </td>
                <td className="py-1 pr-1 text-right">
                  <input
                    type="number"
                    min={1}
                    className={`${CELL} w-16 text-right`}
                    value={r.target}
                    onChange={(e) =>
                      set(i, { target: Number(e.target.value) || 0 })
                    }
                  />
                </td>
                <td className="py-1 pr-1 text-right">
                  <input
                    type="number"
                    min={0}
                    className={`${CELL} w-24 text-right`}
                    value={r.reward_spark}
                    onChange={(e) =>
                      set(i, { reward_spark: Number(e.target.value) || 0 })
                    }
                  />
                </td>
                <td className="py-1 pr-1">
                  <select
                    className={CELL}
                    value={r.item_key}
                    onChange={(e) => set(i, { item_key: e.target.value })}
                  >
                    <option value="">—</option>
                    {items.map((it) => (
                      <option key={it.key} value={it.key}>
                        {it.name}
                      </option>
                    ))}
                  </select>
                </td>
                <td className="py-1 pr-1 text-right">
                  <input
                    type="number"
                    min={1}
                    className={`${CELL} w-14 text-right`}
                    value={r.item_qty}
                    onChange={(e) =>
                      set(i, { item_qty: Number(e.target.value) || 1 })
                    }
                  />
                </td>
                <td className="py-1 text-center">
                  <input
                    type="checkbox"
                    checked={r.enabled}
                    onChange={(e) => set(i, { enabled: e.target.checked })}
                  />
                </td>
                <td className="py-1 pl-1 text-right">
                  <button
                    type="button"
                    className={BTN}
                    disabled={busy !== null}
                    onClick={() => void save(r)}
                  >
                    {busy === r.code ? t.saving : t.save}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {msg && <p className="text-[11px] text-sub">{msg}</p>}
      <p className="text-[11px] text-sub">{t.note}</p>
    </div>
  );
}

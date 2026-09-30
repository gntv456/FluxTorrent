"use client";

/**
 * 农场作物表编辑器（配置面第三件）。
 *
 * 一行作物同时决定两件事：这一档自己的收获期望（产量 ÷ 种子价 × 1.2，
 * 标定上限 = `FARM_BASE_EV`），以及彩蛋池的定标单位（现役最便宜种子价）。
 * 所以保存是服务端两道闸一起过 —— 越标定拒、把池子顶穿也拒，
 * 被拒时原因原样回显、表里一字不改。
 *
 * 收过的作物删不掉（`farm_harvests` 的外键是审计留痕，不该为删一款种子绕过去）；
 * 站长要的其实只是「别再让人买这一款」，那就是**下架**：不进行情、不能播种，
 * 但已种下的地照常可收。
 */

import { useCallback, useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";
import { api, ApiError } from "@/lib/api-client";

interface Crop {
  id: number | null;
  name: string;
  seed_price: number;
  base_yield: number;
  grow_hours: number;
  active: boolean;
  expected_value?: number;
}

const CELL =
  "w-full rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--surface-card)] px-1.5 py-1 text-xs";

const NUM = `${CELL} w-20 text-right`;

const BTN =
  "rounded-full border border-line px-3 py-1 text-[11px] " +
  "font-bold disabled:opacity-50";

export function AdminArcadeCrops({ onChanged }: { onChanged?: () => void }) {
  const { dict, currency } = useI18n();
  const t = dict.adminArcade.crops;
  const [rows, setRows] = useState<Crop[]>([]);
  const [unit, setUnit] = useState(0);
  const [cap, setCap] = useState(0);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [pending, setPending] = useState<number | null>(null);

  const load = useCallback(async () => {
    try {
      const d = await api.get<{
        crops: Crop[];
        unit: number;
        cap: number;
      }>("/api/v1/admin/arcade/crops");
      setRows(d.crops ?? []);
      setUnit(d.unit ?? 0);
      setCap(d.cap ?? 0);
      return true;
    } catch {
      setMsg(t.loadFail);
      return false;
    }
  }, [t.loadFail]);

  useEffect(() => {
    void load();
  }, [load]);

  const set = (i: number, patch: Partial<Crop>) =>
    setRows((rs) => rs.map((r, k) => (k === i ? { ...r, ...patch } : r)));

  function keyOf(r: Crop, i: number): string {
    return r.id === null ? `new-${i}` : String(r.id);
  }

  async function save(r: Crop, i: number) {
    const k = keyOf(r, i);
    setBusy(k);
    setMsg(null);
    try {
      await api.post("/api/v1/admin/arcade/crop", r);
      setMsg(t.saved.replace("{n}", r.name));
    } catch (e) {
      // 跨表回查的拒绝原因就是要给站长看的那句话
      setMsg(e instanceof ApiError ? e.message : t.saveFail);
    }
    await load();
    onChanged?.();
    setBusy(null);
  }

  async function del(r: Crop) {
    if (r.id === null) return;
    if (pending !== r.id) {
      setPending(r.id);
      return;
    }
    setPending(null);
    setBusy(`del-${r.id}`);
    setMsg(null);
    try {
      await api.del(`/api/v1/admin/arcade/crop/${r.id}`);
      setMsg(t.deleted.replace("{n}", r.name));
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.delFail);
    }
    await load();
    onChanged?.();
    setBusy(null);
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h3 className="text-sm font-bold">{t.title}</h3>
        <span className="text-[11px] text-sub">
          {t.unitLine.replace("{n}", String(unit)).replace("{magic}", currency)}
          {cap > 0 && ` · ${t.capLine.replace("{c}", cap.toFixed(2))}`}
        </span>
      </div>

      <div className="overflow-x-auto">
        <table className="nexus-table w-full text-xs">
          <thead>
            <tr className="text-left text-sub">
              <th className="py-1">{t.colName}</th>
              <th className="py-1 text-right">{t.colSeed}</th>
              <th className="py-1 text-right">{t.colYield}</th>
              <th className="py-1 text-right">{t.colGrow}</th>
              <th className="py-1 text-right">{t.colEv}</th>
              <th className="py-1 text-center">{t.colActive}</th>
              <th className="py-1" />
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => {
              const k = keyOf(r, i);
              const over = (r.expected_value ?? 0) > cap && cap > 0;
              return (
                <tr key={k} className="border-t border-line align-top">
                  <td className="py-1 pr-1">
                    <input
                      className={CELL}
                      value={r.name}
                      onChange={(e) => set(i, { name: e.target.value })}
                    />
                  </td>
                  <td className="py-1 pr-1 text-right">
                    <input
                      type="number"
                      min={1}
                      className={NUM}
                      value={r.seed_price}
                      onChange={(e) =>
                        set(i, {
                          seed_price: Number(e.target.value) || 0,
                        })
                      }
                    />
                  </td>
                  <td className="py-1 pr-1 text-right">
                    <input
                      type="number"
                      min={1}
                      className={NUM}
                      value={r.base_yield}
                      onChange={(e) =>
                        set(i, {
                          base_yield: Number(e.target.value) || 0,
                        })
                      }
                    />
                  </td>
                  <td className="py-1 pr-1 text-right">
                    <input
                      type="number"
                      min={1}
                      className={NUM}
                      value={r.grow_hours}
                      onChange={(e) =>
                        set(i, {
                          grow_hours: Number(e.target.value) || 0,
                        })
                      }
                    />
                  </td>
                  <td
                    className={`num py-1 pr-1 text-right ${
                      over ? "text-[var(--coral)]" : "text-sub"
                    }`}
                  >
                    {r.expected_value === undefined
                      ? "-"
                      : r.expected_value.toFixed(3)}
                  </td>
                  <td className="py-1 text-center">
                    <input
                      type="checkbox"
                      checked={r.active}
                      onChange={(e) => set(i, { active: e.target.checked })}
                    />
                  </td>
                  <td className="py-1 pl-1 text-right whitespace-nowrap">
                    {r.id !== null && (
                      <button
                        type="button"
                        className={BTN}
                        disabled={busy !== null}
                        onClick={() => void del(r)}
                      >
                        {pending === r.id ? t.confirmDel : t.del}
                      </button>
                    )}
                    <button
                      type="button"
                      className={BTN}
                      disabled={busy !== null}
                      onClick={() => void save(r, i)}
                    >
                      {busy === k ? t.saving : t.save}
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      <button
        type="button"
        className={`${BTN} self-start`}
        disabled={busy !== null}
        onClick={() =>
          setRows((rs) => [
            ...rs,
            {
              id: null,
              name: t.newRow,
              seed_price: 100,
              base_yield: 75,
              grow_hours: 24,
              active: true,
            },
          ])
        }
      >
        {t.addRow}
      </button>

      {msg && (
        <p
          className={`rounded-[var(--r-sm)] p-2 text-xs ${
            msg.startsWith(t.saved.slice(0, 2)) ||
            msg.startsWith(t.deleted.slice(0, 2))
              ? "bg-mint-soft text-ink"
              : "bg-sun-soft text-ink"
          }`}
        >
          {msg}
        </p>
      )}
      <p className="text-[11px] text-sub">{t.note}</p>
    </div>
  );
}

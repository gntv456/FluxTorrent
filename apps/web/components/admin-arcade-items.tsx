"use client";

/**
 * 物品目录编辑器（配置面第二件）。
 *
 * anchor **只读**：它是派生量（商店价或「该物品能兑换到的最值钱东西」倒推），
 * 手填 anchor 就是 EV 守卫的后门。改一个 anchor 会连带改掉所有引用它的池，
 * 所以这一列由服务端的跨池回查把关 —— 被拒时原因原样回显，表里一字不改。
 */

import { useCallback, useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";
import { api, ApiError } from "@/lib/api-client";
import { AdminArcadeItemsNew } from "./admin-arcade-items-new";

interface Item {
  key: string;
  name: string;
  kind: string;
  anchor: number;
  anchor_src: string;
  unlimited: boolean;
  stock: number;
  per_user: number;
  icon: string;
  enabled: boolean;
}

const CELL =
  "w-full rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--surface-card)] px-1.5 py-1 text-xs";

const BTN =
  "rounded-full border border-line px-3 py-1 text-[11px] " +
  "font-bold disabled:opacity-50";

const LBL = "mt-1 flex items-center justify-end gap-1 text-[10px]";

export function AdminArcadeItems({
  onChanged,
}: {
  onChanged?: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.adminArcade.items;
  const [rows, setRows] = useState<Item[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [pending, setPending] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const d = await api.get<{ items?: Item[] }>(
        "/api/v1/admin/arcade/overview",
      );
      setRows(d.items ?? []);
    } catch {
      setMsg(t.loadFail);
    }
  }, [t.loadFail]);

  useEffect(() => {
    void load();
  }, [load]);

  const set = (i: number, patch: Partial<Item>) =>
    setRows((rs) => rs.map((r, k) => (k === i ? { ...r, ...patch } : r)));

  async function del(r: Item) {
    // 两步确认：第一下只把按钮变成「确认删除」，不弹窗
    if (pending !== r.key) {
      setPending(r.key);
      return;
    }
    setPending(null);
    setBusy(r.key);
    setMsg(null);
    try {
      await api.del(
        `/api/v1/admin/arcade/items/${encodeURIComponent(r.key)}`,
      );
      setMsg(t.deleted.replace("{k}", r.key));
    } catch (e) {
      // 服务端会点名「被哪个池的哪一档引用」或「已发放过多少件」
      setMsg(e instanceof ApiError ? e.message : t.delFail);
    }
    await load();
    onChanged?.();
    setBusy(null);
  }

  async function save(r: Item) {
    setBusy(r.key);
    setMsg(null);
    try {
      await api.post("/api/v1/admin/arcade/items", r);
      setMsg(t.saved.replace("{k}", r.key));
    } catch (e) {
      // 跨池回查的拒绝原因就是要给站长看的那句话
      setMsg(e instanceof ApiError ? e.message : t.saveFail);
    }
    await load();
    onChanged?.();
    setBusy(null);
  }

  return (
    <div className="flex flex-col gap-3">
      <h3 className="text-sm font-bold">{t.title}</h3>

      <AdminArcadeItemsNew
        onCreated={() => void load()}
        busy={busy !== null}
      />

      <div className="overflow-x-auto">
        <table className="nexus-table w-full text-xs">
          <thead>
            <tr className="text-left text-sub">
              <th className="py-1">{t.colIcon}</th>
              <th className="py-1">{t.colName}</th>
              <th className="py-1">{t.colKind}</th>
              <th className="py-1 text-right">{t.colAnchor}</th>
              <th className="py-1 text-right">{t.colStock}</th>
              <th className="py-1 text-right">{t.colPerUser}</th>
              <th className="py-1 text-center">{t.colOn}</th>
              <th className="py-1" />
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr key={r.key} className="border-t border-line align-top">
                <td className="py-1 pr-1">
                  <input
                    className={`${CELL} w-12 text-center`}
                    value={r.icon}
                    onChange={(e) => set(i, { icon: e.target.value })}
                  />
                </td>
                <td className="py-1 pr-1">
                  <input
                    className={CELL}
                    value={r.name}
                    onChange={(e) => set(i, { name: e.target.value })}
                  />
                  <span className="text-[10px] text-sub">{r.key}</span>
                </td>
                <td className="py-1 pr-1">
                  <input
                    className={`${CELL} w-24`}
                    value={r.kind}
                    onChange={(e) => set(i, { kind: e.target.value })}
                  />
                </td>
                <td className="py-1 pr-1 text-right">
                  <input
                    type="number"
                    min={0}
                    className={CELL + " w-24 text-right"}
                    value={r.anchor}
                    onChange={(e) =>
                      set(i, {
                        anchor: Number(e.target.value) || 0,
                        anchor_src: "declared",
                      })
                    }
                  />
                  <span className="block text-[10px] text-sub">
                    {r.anchor_src}
                  </span>
                </td>
                <td className="py-1 pr-1 text-right">
                  {r.unlimited ? (
                    <span className="text-[11px] text-sub">{t.unlimited}</span>
                  ) : (
                    <input
                      type="number"
                      min={0}
                      className={`${CELL} w-20 text-right`}
                      value={r.stock}
                      onChange={(e) =>
                        set(i, { stock: Number(e.target.value) || 0 })
                      }
                    />
                  )}
                  <label
                    className={LBL}
                  >
                    <input
                      type="checkbox"
                      checked={r.unlimited}
                      onChange={(e) => set(i, { unlimited: e.target.checked })}
                    />
                    {t.colUnlimited}
                  </label>
                </td>
                <td className="py-1 pr-1 text-right">
                  <input
                    type="number"
                    min={1}
                    className={`${CELL} w-16 text-right`}
                    value={r.per_user}
                    onChange={(e) =>
                      set(i, { per_user: Number(e.target.value) || 1 })
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
                    onClick={() => void del(r)}
                  >
                    {pending === r.key ? t.confirmDel : t.del}
                  </button>
                  <button
                    type="button"
                    className={BTN}

                    disabled={busy !== null}
                    onClick={() => void save(r)}
                  >
                    {busy === r.key ? t.saving : t.save}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {msg && (
        <p
          className={`rounded-[var(--r-sm)] p-2 text-xs ${
            msg.startsWith(t.saved.slice(0, 3))
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

"use client";

import { useI18n } from "@/i18n/client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { MedalIcon, usableAssetUrl } from "@/components/medal-icon";
import { MedalRarityChip, MedalRarityPreview } from "@/components/medal-rarity-chip";
import {
  DEFAULT_MEDAL_RARITIES,
  MEDAL_RARITY_TONES,
  cleanRarityValue,
  isKnownRarity,
  type MedalRarity,
} from "@/lib/medal-rarity";

/** 稀有度下拉里的「自定义…」哨兵值（真实值由旁边的文本框输入） */
const CUSTOM_RARITY = "__custom__";
const RARITY_INP = "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm outline-none focus:border-sky";

/** 第八轮 P2-7：勋章管理（好学站 system/medals 简化口径）
 *  字典 CRUD + 全站持有浏览 + 回收（授予入口在用户详情页） */

interface MedalRow {
  id: number;
  name: string;
  description: string | null;
  price: number | null;
  rarity: string | null;
  limited: boolean;
  get_type: number;
  duration_days: number | null;
  bonus_addition_factor: number | null;
  category_id: number;
  asset_ref: string | null;
  held_count: number;
}

interface UserMedalRow {
  user_id: number;
  username: string;
  medal_id: number;
  medal_name: string;
  source: string;
  wearing: boolean;
  granted_at: string | null;
}

const GET_TYPE: Record<number, string> = { 1: "兑换", 2: "授予", 3: "合成" };

/** 稀有度词表编辑（0143）：键/显示名/配色档/排序；键即 medals.rarity 里存的值。
 *  有勋章在用的条目不许删（后端也拦），改键会连带迁移那些勋章。 */
function RarityDict({
  rows,
  onChanged,
  flash,
}: {
  rows: MedalRarity[];
  onChanged: () => Promise<void> | void;
  flash: (m: string) => void;
}) {
  const blank = { key: null as string | null, value: "", label: "", tone: "sky", sort: 100 };
  const [edit, setEdit] = useState(blank);
  const [busy, setBusy] = useState(false);

  async function save() {
    const value = cleanRarityValue(edit.value);
    if (!value) return flash("稀有度键不能为空（仅字母/数字/下划线/短横线）");
    if (!edit.label.trim()) return flash("显示名不能为空");
    setBusy(true);
    try {
      if (edit.key === null) {
        await api.post("/api/v1/admin/medal-rarities", {
          value,
          label: edit.label.trim(),
          tone: edit.tone,
          sort: edit.sort,
        });
      } else {
        await api.put(`/api/v1/admin/medal-rarities/${encodeURIComponent(edit.key)}`, {
          value,
          label: edit.label.trim(),
          tone: edit.tone,
          sort: edit.sort,
        });
      }
      flash("已保存");
      setEdit(blank);
      await onChanged();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="baozi-panel p-4">
      <div className="mb-2 flex flex-wrap items-baseline gap-2">
        <h3 className="text-sm font-bold">稀有度词表</h3>
        <span className="text-[11px] text-sub">这里的「键」就是勋章上存的值；改键会同步迁移已用它的勋章</span>
      </div>
      <div className="mb-3 flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs">键
          <input
            value={edit.value}
            onChange={(e) => setEdit({ ...edit, value: e.target.value })}
            placeholder="如 mythic"
            className={`${RARITY_INP} w-32`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">显示名
          <input
            value={edit.label}
            onChange={(e) => setEdit({ ...edit, label: e.target.value })}
            placeholder="如 神话"
            className={`${RARITY_INP} w-28`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">配色
          <select value={edit.tone} onChange={(e) => setEdit({ ...edit, tone: e.target.value })} className={RARITY_INP}>
            {MEDAL_RARITY_TONES.map((t) => (
              <option key={t.value} value={t.value}>
                {t.label}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">排序
          <input
            type="number"
            value={edit.sort}
            onChange={(e) => setEdit({ ...edit, sort: Number(e.target.value) })}
            className={`${RARITY_INP} w-20`}
          />
        </label>
        <span className="pb-2">
          <MedalRarityPreview label={edit.label} tone={edit.tone} />
        </span>
        <button className="baozi-button" disabled={busy} onClick={save}>
          {edit.key === null ? "新增" : "保存"}
        </button>
        {edit.key !== null && (
          <button
            className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
            onClick={() => setEdit(blank)}
          >
            取消
          </button>
        )}
      </div>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">键</td>
            <td className="colhead">显示名</td>
            <td className="colhead">配色</td>
            <td className="colhead">排序</td>
            <td className="colhead">使用中</td>
            <td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.value}>
              <td className="font-mono">{r.value}</td>
              <td>{r.label}</td>
              <td>
                <MedalRarityPreview label={r.label} tone={r.tone} />
              </td>
              <td className="num">{r.sort ?? 0}</td>
              <td className="num">{r.used ?? 0}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={() =>
                    setEdit({ key: r.value, value: r.value, label: r.label, tone: r.tone, sort: r.sort ?? 100 })
                  }
                >
                  编辑
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  disabled={busy || (r.used ?? 0) > 0}
                  title={(r.used ?? 0) > 0 ? `仍有 ${r.used} 枚勋章在用，先改掉它们` : "删除"}
                  onClick={async () => {
                    try {
                      await api.del(`/api/v1/admin/medal-rarities/${encodeURIComponent(r.value)}`);
                      flash("已删除");
                      await onChanged();
                    } catch (e) {
                      flash(e instanceof ApiError ? e.message : "删除失败");
                    }
                  }}
                >
                  删除
                </button>
              </td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={6} className="py-4 text-center text-sub">
                暂未配置稀有度
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}

/** 勋章图片预览：图挂了要说清原因（站内图床 /api/v1/attachments 需登录，<img> 拿不到） */
function MedalImagePreview({ src }: { src?: string | null }) {
  const url = usableAssetUrl(src);
  const [err, setErr] = useState(false);
  useEffect(() => {
    setErr(false);
  }, [url]);
  if (!url) return <span className="pb-1 text-[11px] text-sub">未设置图片，展示位回落 🏅</span>;
  if (err)
    return (
      <span className="max-w-xs pb-1 text-[11px] text-danger">
        图片加载失败：站内图床地址（/api/v1/attachments/…）需要登录才能取图，请改填外链 https 地址
      </span>
    );
  return (
    // eslint-disable-next-line @next/next/no-img-element
    <img
      src={url}
      alt="预览"
      className="h-10 w-10 rounded-[var(--r-sm)] border border-line bg-cloud object-cover"
      onError={() => setErr(true)}
    />
  );
}

export function AdminMedals() {
  const { currency } = useI18n();
  const [rows, setRows] = useState<MedalRow[]>([]);
  const [held, setHeld] = useState<UserMedalRow[]>([]);
  const [heldUid, setHeldUid] = useState("");
  const [edit, setEdit] = useState<{ id: number | null; f: Partial<MedalRow> }>({ id: null, f: { name: "", get_type: 2, category_id: 0, limited: false } });
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  /** 稀有度选「自定义…」时为 true（DB 列是自由文本，允许站长新增稀有度） */
  const [customRarity, setCustomRarity] = useState(false);
  /** 稀有度词表（0143）：下拉候选与列表标签都读它 */
  const [rarities, setRarities] = useState<MedalRarity[]>(DEFAULT_MEDAL_RARITIES);

  const flash = (m: string) => { setMsg(m); setTimeout(() => setMsg(null), 3000); };
  const blankForm = { id: null, f: { name: "", get_type: 2, category_id: 0, limited: false } };
  const resetForm = () => { setEdit(blankForm); setCustomRarity(false); };
  // 已有值不在候选表里（历史数据 / 站长自定义）→ 自动落到自定义输入
  const rarityIsCustom = customRarity || (!!edit.f.rarity && !isKnownRarity(rarities, edit.f.rarity));

  const load = useCallback(async () => {
    try {
      setRows(await api.get<MedalRow[]>("/api/v1/admin/medals"));
      // 词表拉不到就用内置兜底，不让整个面板报错
      setRarities(await api.get<MedalRarity[]>("/api/v1/admin/medal-rarities").catch(() => DEFAULT_MEDAL_RARITIES));
      const q = heldUid.trim() ? `?uid=${encodeURIComponent(heldUid.trim())}` : "";
      // 后端返回分页信封 {rows,total,page,per_page}（0096 前是裸数组，双形态兼容）
      const r = await api.get<UserMedalRow[] | { rows: UserMedalRow[] }>(`/api/v1/admin/user-medals${q}`);
      setHeld(Array.isArray(r) ? r : r.rows);
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "加载失败");
    }
  }, [heldUid]);
  useEffect(() => { load(); }, [load]);

  async function save() {
    setBusy(true);
    try {
      if (edit.id === null) await api.post("/api/v1/admin/medals", edit.f);
      else await api.put(`/api/v1/admin/medals/${edit.id}`, edit.f);
      flash("已保存");
      resetForm();
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  const inp = "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm outline-none focus:border-sky";

  return (
    <div className="flex flex-col gap-3">
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}

      <section className="baozi-panel cmgmt-form p-4">
        <h2 className="mb-2 text-base font-bold">{edit.id === null ? "新建勋章" : `编辑勋章 #${edit.id}`}</h2>
        {/* 勋章图片（medals.asset_ref）：此前后端能存、表单没入口 → 全站只能画 🏅 */}
        <div className="mb-3 flex flex-wrap items-center gap-3 border-b border-line pb-3">
          <label className="flex flex-col gap-1 text-xs">勋章图片 URL
            <input
              value={edit.f.asset_ref ?? ""}
              onChange={(e) => setEdit({ ...edit, f: { ...edit.f, asset_ref: e.target.value || null } })}
              placeholder="https://…/medal.png"
              className={`${inp} w-80`}
            />
          </label>
          <MedalImagePreview src={edit.f.asset_ref} />
          {edit.f.asset_ref ? (
            <button
              type="button"
              className="min-h-[32px] rounded-full border border-line px-3 text-xs font-bold"
              onClick={() => setEdit({ ...edit, f: { ...edit.f, asset_ref: null } })}
            >
              清除图片
            </button>
          ) : null}
        </div>
        <div className="flex flex-wrap items-end gap-2">
          <label className="flex flex-col gap-1 text-xs">名称
            <input value={edit.f.name ?? ""} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, name: e.target.value } })} className={`${inp} w-32`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">说明
            <input value={edit.f.description ?? ""} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, description: e.target.value } })} className={`${inp} w-48`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">获取方式
            <select value={edit.f.get_type ?? 2} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, get_type: Number(e.target.value) } })} className={inp}>
              <option value={1}>兑换</option>
              <option value={2}>授予</option>
              <option value={3}>合成</option>
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs">价格({currency})
            <input type="number" value={edit.f.price ?? ""} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, price: e.target.value ? Number(e.target.value) : null } })} className={`${inp} w-24`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">魔力加成(%)
            <input type="number" value={edit.f.bonus_addition_factor ?? ""} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, bonus_addition_factor: e.target.value ? Number(e.target.value) : null } })} className={`${inp} w-24`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">有效期(天,空=永久)
            <input type="number" value={edit.f.duration_days ?? ""} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, duration_days: e.target.value ? Number(e.target.value) : null } })} className={`${inp} w-24`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">稀有度
            <select
              value={rarityIsCustom ? CUSTOM_RARITY : (edit.f.rarity ?? "")}
              onChange={(e) => {
                const v = e.target.value;
                if (v === CUSTOM_RARITY) {
                  setCustomRarity(true);
                  setEdit({ ...edit, f: { ...edit.f, rarity: null } });
                } else {
                  setCustomRarity(false);
                  setEdit({ ...edit, f: { ...edit.f, rarity: v || null } });
                }
              }}
              className={inp}
            >
              <option value="">未设置</option>
              {rarities.map((r) => (
                <option key={r.value} value={r.value}>{`${r.label}（${r.value}）`}</option>
              ))}
              <option value={CUSTOM_RARITY}>自定义…</option>
            </select>
          </label>
          {rarityIsCustom && (
            <label className="flex flex-col gap-1 text-xs">自定义稀有度
              <input
                value={edit.f.rarity ?? ""}
                onChange={(e) => setEdit({ ...edit, f: { ...edit.f, rarity: e.target.value || null } })}
                placeholder="如 super-rare"
                className={`${inp} w-32`}
              />
            </label>
          )}
          <label className="flex flex-col gap-1 text-xs">分组
            <input type="number" value={edit.f.category_id ?? 0} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, category_id: Number(e.target.value) } })} className={`${inp} w-16`} />
          </label>
          <label className="flex items-center gap-1 pb-2 text-xs">
            <input type="checkbox" checked={Boolean(edit.f.limited)} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, limited: e.target.checked } })} />限定
          </label>
          <button className="baozi-button" disabled={busy || !String(edit.f.name ?? "").trim()} onClick={save}>保存</button>
          {edit.id !== null && <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold" onClick={resetForm}>取消</button>}
        </div>
      </section>

      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">ID</td><td className="colhead">图标</td><td className="colhead">名称</td><td className="colhead">获取</td>
            <td className="colhead">稀有度</td>
            <td className="colhead">价格</td><td className="colhead">加成%</td><td className="colhead">有效期</td>
            <td className="colhead">持有数</td><td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((m) => (
            <tr key={m.id}>
              <td className="num">{m.id}</td>
              <td>
                <MedalIcon src={m.asset_ref} size={28} title={m.name} />
              </td>
              <td className="font-bold">{m.name}{m.limited && <span className="ml-1 rounded-full bg-coral/20 px-1.5 text-[10px] text-danger">限定</span>}</td>
              <td>{GET_TYPE[m.get_type] ?? m.get_type}</td>
              <td>
                {m.rarity ? <MedalRarityChip list={rarities} value={m.rarity} /> : "—"}
              </td>
              <td className="num">{m.price ?? "—"}</td>
              <td className="num">{m.bonus_addition_factor ?? 0}</td>
              <td className="num">{m.duration_days ?? "永久"}</td>
              <td className="num">{m.held_count}</td>
              <td className="text-right">
                <button className="cmgmt-act" onClick={() => { setEdit({ id: m.id, f: { ...m } }); setCustomRarity(false); }}>编辑</button>
                <button className="cmgmt-act cmgmt-act--danger" disabled={busy}
                  onClick={async () => {
                    try { await api.del(`/api/v1/admin/medals/${m.id}`); flash("已删除"); await load(); }
                    catch (e) { flash(e instanceof ApiError ? e.message : "删除失败"); }
                  }}>删除</button>
              </td>
            </tr>
          ))}
          {rows.length === 0 && <tr><td colSpan={10} className="py-6 text-center text-sub">暂无勋章</td></tr>}
        </tbody>
      </table>

      <RarityDict rows={rarities} onChanged={load} flash={flash} />

      <section className="baozi-panel p-4">
        <div className="mb-2 flex items-end gap-2">
          <h3 className="text-sm font-bold">持有浏览 / 回收</h3>
          <input value={heldUid} onChange={(e) => setHeldUid(e.target.value)} placeholder="按用户 UID 过滤" className="min-h-[32px] w-40 rounded-full border border-line px-3 text-xs" />
        </div>
        <table className="nexus-table text-xs">
          <thead>
            <tr><td className="colhead">用户</td><td className="colhead">勋章</td><td className="colhead">来源</td><td className="colhead">佩戴</td><td className="colhead text-right">操作</td></tr>
          </thead>
          <tbody>
            {held.map((h) => (
              <tr key={`${h.user_id}-${h.medal_id}`}>
                <td><a href={`/admin/users/${h.user_id}`} className="font-bold text-link">{h.username}</a></td>
                <td>{h.medal_name}</td>
                <td>{h.source}</td>
                <td>{h.wearing ? "佩戴中" : "—"}</td>
                <td className="text-right">
                  <button className="cmgmt-act cmgmt-act--danger" disabled={busy}
                    onClick={async () => {
                      try {
                        await api.post("/api/v1/admin/user-medals/delete", { user_id: h.user_id, medal_id: h.medal_id });
                        flash("已回收");
                        await load();
                      } catch (e) { flash(e instanceof ApiError ? e.message : "回收失败"); }
                    }}>回收</button>
                </td>
              </tr>
            ))}
            {held.length === 0 && <tr><td colSpan={5} className="py-4 text-center text-sub">暂无持有记录</td></tr>}
          </tbody>
        </table>
      </section>
    </div>
  );
}

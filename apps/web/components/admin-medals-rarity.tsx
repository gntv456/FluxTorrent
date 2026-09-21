"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { usableAssetUrl } from "@/components/medal-icon";
import { MedalRarityPreview } from "@/components/medal-rarity-chip";
import {
  MEDAL_RARITY_TONES,
  cleanRarityValue,
  type MedalRarity,
} from "@/lib/medal-rarity";

/** 勋章管理的稀有度与预览部件（从 admin-medals.tsx 按域拆出）：
 *  RarityDict 稀有度词表编辑 + MedalImagePreview 图片预览。 */

/** 表单输入基线样式（原 admin-medals.tsx 的 RARITY_INP 常量） */
const RARITY_INP =
  "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud " +
  "px-2 text-sm outline-none focus:border-sky";

/** 稀有度下拉里的「自定义…」哨兵值（真实值由旁边的文本框输入） */
export const CUSTOM_RARITY = "__custom__";

/** 稀有度词表编辑（0143）：键/显示名/配色档/排序；键即 medals.rarity 里存的值。
 *  有勋章在用的条目不许删（后端也拦），改键会连带迁移那些勋章。 */
export function RarityDict({
  rows,
  onChanged,
  flash,
}: {
  rows: MedalRarity[];
  onChanged: () => Promise<void> | void;
  flash: (m: string) => void;
}) {
  const blank = {
    key: null as string | null,
    value: "",
    label: "",
    tone: "sky",
    sort: 100,
  };
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
        await api.put(
          `/api/v1/admin/medal-rarities/${encodeURIComponent(edit.key)}`,
          {
            value,
            label: edit.label.trim(),
            tone: edit.tone,
            sort: edit.sort,
          },
        );
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
        <span className="text-[11px] text-sub">
          这里的「键」就是勋章上存的值；改键会同步迁移已用它的勋章
        </span>
      </div>
      <div className="mb-3 flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs">
          键
          <input
            value={edit.value}
            onChange={(e) => setEdit({ ...edit, value: e.target.value })}
            placeholder="如 mythic"
            className={`${RARITY_INP} w-32`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          显示名
          <input
            value={edit.label}
            onChange={(e) => setEdit({ ...edit, label: e.target.value })}
            placeholder="如 神话"
            className={`${RARITY_INP} w-28`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          配色
          <select
            value={edit.tone}
            onChange={(e) => setEdit({ ...edit, tone: e.target.value })}
            className={RARITY_INP}
          >
            {MEDAL_RARITY_TONES.map((t) => (
              <option key={t.value} value={t.value}>
                {t.label}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          排序
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
                    setEdit({
                      key: r.value,
                      value: r.value,
                      label: r.label,
                      tone: r.tone,
                      sort: r.sort ?? 100,
                    })
                  }
                >
                  编辑
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  disabled={busy || (r.used ?? 0) > 0}
                  title={
                    (r.used ?? 0) > 0
                      ? `仍有 ${r.used} 枚勋章在用，先改掉它们`
                      : "删除"
                  }
                  onClick={async () => {
                    try {
                      await api.del(
                        `/api/v1/admin/medal-rarities/${encodeURIComponent(r.value)}`,
                      );
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
export function MedalImagePreview({ src }: { src?: string | null }) {
  const url = usableAssetUrl(src);
  const [err, setErr] = useState(false);
  useEffect(() => {
    setErr(false);
  }, [url]);
  if (!url)
    return (
      <span className="pb-1 text-[11px] text-sub">
        未设置图片，展示位回落 🏅
      </span>
    );
  if (err)
    return (
      <span className="max-w-xs pb-1 text-[11px] text-danger">
        图片加载失败：站内图床地址（/api/v1/attachments/…）需要登录才能取图，请改填外链
        https 地址
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

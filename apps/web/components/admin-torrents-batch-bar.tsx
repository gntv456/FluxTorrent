"use client";

/**
 * 后台种子管理·批量工具条（从 components/admin-torrents-list.tsx
 * 按域拆出）：置顶/促销/推荐/打标/H&R/改分类/删除等批量动作条。
 * 好学站 torrent/torrents 批量动作口径。
 */

import type { CatRow, TagRow } from "./admin-torrents-shared";
import { promoLabels } from "./admin-torrents-shared";
import { useI18n } from "@/i18n/client";

interface BatchBarProps {
  busy: boolean;
  selCount: number;
  tags: TagRow[];
  cats: CatRow[];
  posUntil: string;
  setPosUntil: (v: string) => void;
  promoKind: string;
  setPromoKind: (v: string) => void;
  promoHours: string;
  setPromoHours: (v: string) => void;
  pickType: string;
  setPickType: (v: string) => void;
  tagIds: Set<number>;
  setTagIds: (v: Set<number>) => void;
  batchCat: string;
  setBatchCat: (v: string) => void;
  batch: (action: string, extra?: Record<string, unknown>) => Promise<void>;
}

/** 批量工具条输入/下拉统一样式 */
const BATCH_INPUT_CLS =
  "min-h-[36px] rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--surface-card)] px-2 text-xs";
/** 批量工具条通用的描边小按钮样式 */
const OUTLINE_BTN =
  "min-h-[36px] rounded-full border border-line px-4 text-xs " +
  "font-bold disabled:opacity-50";
/** 批量工具条通用的实底小按钮样式（天蓝/珊瑚红） */
const SKY_BTN =
  "min-h-[36px] rounded-full bg-sky px-4 text-xs font-bold " +
  "text-white disabled:opacity-50";
const CORAL_BTN =
  "min-h-[36px] rounded-full bg-coral px-4 text-xs font-bold " +
  "text-white disabled:opacity-50";

export function BatchBar(props: BatchBarProps) {
  const {
    busy,
    selCount,
    tags,
    cats,
    posUntil,
    setPosUntil,
    promoKind,
    setPromoKind,
    promoHours,
    setPromoHours,
    pickType,
    setPickType,
    tagIds,
    setTagIds,
    batchCat,
    setBatchCat,
    batch,
  } = props;
  const { dict } = useI18n();
  const PROMO_LABEL = promoLabels(dict.adminTorrents.promo);
  return (
    <section className="baozi-panel flex flex-wrap items-end gap-3 p-3">
      <p className="w-full text-xs font-bold text-sub">
        批量操作（已选 {selCount} 个，勾选下方列表后执行；单批最多 500）
      </p>
      <label className="flex flex-col gap-1 text-xs">
        置顶截止
        <input
          type="datetime-local"
          value={posUntil}
          onChange={(e) => setPosUntil(e.target.value)}
          className={BATCH_INPUT_CLS}
        />
      </label>
      <button
        disabled={busy}
        onClick={() =>
          batch("sticky", {
            pos_state: 1,
            pos_state_until: posUntil ? new Date(posUntil).toISOString() : null,
          })
        }
        className={SKY_BTN}
      >
        置顶
      </button>
      <button
        disabled={busy}
        onClick={() => batch("sticky", { pos_state: 0 })}
        className={OUTLINE_BTN}
      >
        取消置顶
      </button>
      <label className="flex flex-col gap-1 text-xs">
        促销类型
        <select
          value={promoKind}
          onChange={(e) => setPromoKind(e.target.value)}
          className={BATCH_INPUT_CLS}
        >
          {Object.entries(PROMO_LABEL).map(([k, l]) => (
            <option key={k} value={k}>
              {l}
            </option>
          ))}
        </select>
      </label>
      <label className="flex flex-col gap-1 text-xs">
        时长(h)
        <input
          type="number"
          value={promoHours}
          onChange={(e) => setPromoHours(e.target.value)}
          className={`w-16 ${BATCH_INPUT_CLS}`}
        />
      </label>
      <button
        disabled={busy}
        onClick={() =>
          batch("promo", {
            promo_kind: promoKind,
            promo_until: new Date(
              Date.now() + (Number(promoHours) || 48) * 3600e3,
            ).toISOString(),
          })
        }
        className={SKY_BTN}
      >
        设促销
      </button>
      <label className="flex flex-col gap-1 text-xs">
        推荐
        <select
          value={pickType}
          onChange={(e) => setPickType(e.target.value)}
          className={BATCH_INPUT_CLS}
        >
          <option value="0">取消</option>
          <option value="1">推荐</option>
          <option value="2">经典</option>
        </select>
      </label>
      <button
        disabled={busy}
        onClick={() => batch("recommend", { pick_type: Number(pickType) })}
        className={OUTLINE_BTN}
      >
        设推荐
      </button>
      <label className="flex flex-wrap items-center gap-1 pb-1 text-xs">
        标签
        {tags
          .filter((t) => t.enabled)
          .map((t) => (
            <label key={t.id} className="flex items-center gap-0.5">
              <input
                type="checkbox"
                checked={tagIds.has(t.id)}
                onChange={(e) => {
                  const n = new Set(tagIds);
                  if (e.target.checked) n.add(t.id);
                  else n.delete(t.id);
                  setTagIds(n);
                }}
              />
              {t.name}
            </label>
          ))}
      </label>
      <button
        disabled={busy || tagIds.size === 0}
        onClick={() => batch("set_tags", { tag_ids: [...tagIds] })}
        className={OUTLINE_BTN}
      >
        设置标签
      </button>
      <button
        disabled={busy}
        onClick={() => batch("clear_tags", { tag_ids: [...tagIds] })}
        className={OUTLINE_BTN}
      >
        清除标签
      </button>
      <button
        disabled={busy}
        onClick={() => batch("hr")}
        className={OUTLINE_BTN}
      >
        标记H&R
      </button>
      <button
        disabled={busy}
        onClick={() => batch("unhr")}
        className={OUTLINE_BTN}
      >
        取消H&R
      </button>
      <label className="flex flex-col gap-1 text-xs">
        改分类
        <select
          value={batchCat}
          onChange={(e) => setBatchCat(e.target.value)}
          className={BATCH_INPUT_CLS}
        >
          <option value="">（不改）</option>
          {cats.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
      </label>
      <button
        disabled={busy || !batchCat}
        onClick={() =>
          batch("change_category", { category_id: Number(batchCat) })
        }
        className={OUTLINE_BTN}
      >
        改分类
      </button>
      <button
        disabled={busy}
        onClick={() => {
          if (window.confirm(`确认删除所选 ${selCount} 个种子？（软删除）`))
            batch("delete");
        }}
        className={CORAL_BTN}
      >
        删除已选
      </button>
    </section>
  );
}

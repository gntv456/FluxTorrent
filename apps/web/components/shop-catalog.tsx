"use client";

import { useMemo, useState } from "react";
import type { ShopItem } from "@fluxtorrent/domain-types";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import { BuyButton } from "@/components/buy-button";
import { FramePreview } from "@/components/frame-preview";

// 魔力商店目录（客户端）：按 kind 归 5 组 + 组内筛选 + 余额预判。
// 分组映射与图标是**前端常量**：新增 kind 只需补一行，未映射的落「其他」组，
// 不会再出现「14 种商品都叫站点特权」的老问题。

type Group = "upload" | "voucher" | "right" | "look" | "gift" | "other";

const GROUP_ORDER: Group[] = [
  "upload",
  "voucher",
  "right",
  "look",
  "gift",
  "other",
];

const KIND_GROUP: Record<string, Group> = {
  upload_credit: "upload",
  voucher_free: "voucher",
  voucher_neutral: "voucher",
  vip: "right",
  app_vip: "right",
  ad_free: "right",
  invite: "right",
  temp_invite: "right",
  rename_card: "right",
  makeup_card: "right",
  custom_title: "right",
  avatar_frame: "look",
  animated_avatar: "look",
  rainbow_id: "look",
  rainbow_name: "look",
  charity: "gift",
  gift_spark: "gift",
};

/** 图标是 UI 装饰（非文案），不进字典；未收录的 kind 用分组默认图 */
const KIND_ICON: Record<string, string> = {
  upload_credit: "📦",
  voucher_free: "🎟️",
  voucher_neutral: "🎫",
  vip: "👑",
  app_vip: "📱",
  ad_free: "🔕",
  invite: "✉️",
  temp_invite: "⏳",
  rename_card: "🏷️",
  makeup_card: "📌",
  custom_title: "💬",
  avatar_frame: "🖼️",
  animated_avatar: "✨",
  rainbow_id: "🌈",
  rainbow_name: "🪄",
  charity: "🤝",
  gift_spark: "🎁",
};

const GROUP_ICON: Record<Group, string> = {
  upload: "📦",
  voucher: "🎟️",
  right: "👑",
  look: "🖼️",
  gift: "🎁",
  other: "🧩",
};

/** 从商品名解析容量（端点不返回 config，故只能用名字；解析失败只是不显示单价） */
function gbOf(name: string): number | null {
  const m = /(\d+(?:\.\d+)?)\s*GB/i.exec(name);
  if (!m) return null;
  const n = Number(m[1]);
  return Number.isFinite(n) && n > 0 ? n : null;
}

export function ShopCatalog({
  items,
  balance,
}: {
  items: ShopItem[];
  /** 当前用户魔力余额；null = 未登录/取不到 → 不做预判，交给后端校验 */
  balance: number | null;
}) {
  const { dict, locale, currency } = useI18n();
  const t = dict.shop;
  const hints = t.hints as Record<string, string>;
  const [grp, setGrp] = useState<Group | null>(null);
  const num = (n: number) => n.toLocaleString(dateLocale(locale));

  const grouped = useMemo(() => {
    const m = new Map<Group, ShopItem[]>();
    for (const it of items) {
      const g = KIND_GROUP[it.kind] ?? "other";
      const arr = m.get(g) ?? [];
      arr.push(it);
      m.set(g, arr);
    }
    return GROUP_ORDER.filter((g) => m.has(g)).map((g) => ({
      g,
      list: m.get(g) as ShopItem[],
    }));
  }, [items]);

  // 组内单位价最低的一件 → 「单位最划算」角标
  const bestId = useMemo(() => {
    const up = items.filter((i) => i.kind === "upload_credit");
    let best: { id: number; unit: number } | null = null;
    for (const i of up) {
      const gb = gbOf(i.name);
      if (!gb) continue;
      const unit = i.price / gb;
      if (!best || unit < best.unit) best = { id: i.id, unit };
    }
    return best?.id ?? null;
  }, [items]);

  const groupLabel = (g: Group) => {
    const names: Record<Group, string> = {
      upload: t.grpUpload,
      voucher: t.grpVoucher,
      right: t.grpRight,
      look: t.grpLook,
      gift: t.grpGift,
      other: t.grpOther,
    };
    return names[g];
  };

  const shown = grouped.filter((x) => !grp || x.g === grp);

  const card = (it: ShopItem) => {
    const g = KIND_GROUP[it.kind] ?? "other";
    const gb = it.kind === "upload_credit" ? gbOf(it.name) : null;
    const unit = gb ? Math.round(it.price / gb) : null;
    const short = balance !== null && balance < it.price;
    return (
      <article className="gcard" key={it.id}>
        {it.id === bestId && (
          <span className="gcard-best">{t.bestValue}</span>
        )}
        <div className="gcard-bd">
          <span className="gicon" data-group={g}>
            {it.kind === "animated_avatar"
              ? (it.config?.effect ?? "✨")
              : (KIND_ICON[it.kind] ?? GROUP_ICON[g])}
          </span>
          <div className="txt">
            <h3>{it.name}</h3>
            <span className="kind">{it.kind.toUpperCase()}</span>
            {hints[it.kind] && (
              <span className="hint">
                {fmt(hints[it.kind], { magic: currency })}
              </span>
            )}
          </div>
          {it.kind === "avatar_frame" && it.config?.frame_id != null && (
            <FramePreview frameId={it.config.frame_id} />
          )}
        </div>
        <div className="gcard-ft">
          <div className="pricetag">
            <b>{num(it.price)}</b>
            <span>{currency}</span>
          </div>
          {unit !== null && (
            <span className="unit">{`${num(unit)} /GB`}</span>
          )}
          <div className="acts">
            <BuyButton
              itemId={it.id}
              unitPrice={it.price}
              stackable={it.config?.stackable === true}
              affordable={!short}
              shortBy={
                short && balance !== null
                  ? fmt(t.shortBy, { n: num(it.price - balance) })
                  : null
              }
              balance={balance}
            />
          </div>
        </div>
      </article>
    );
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="pgtools">
        <div className="tide-tabs" role="tablist">
          <button
            type="button"
            role="tab" className="tide-tab"
            aria-selected={grp === null}
            onClick={() => setGrp(null)}
          >
            {t.all} {items.length}
          </button>
          {grouped.map((x) => (
            <button
              key={x.g}
              type="button"
              role="tab" className="tide-tab"
              aria-selected={grp === x.g}
              onClick={() => setGrp(x.g)}
            >
              {groupLabel(x.g)} {x.list.length}
            </button>
          ))}
        </div>
      </div>

      {shown.map((x) => (
        <section className="baozi-panel" key={x.g}>
          <div className="baozi-panel__head">
            <h2>{groupLabel(x.g)}</h2>
            <span className="text-xs text-sub">
              {fmt(t.groupCount, { n: x.list.length })}
            </span>
          </div>
          <div className="shop-grid">{x.list.map(card)}</div>
        </section>
      ))}
    </div>
  );
}

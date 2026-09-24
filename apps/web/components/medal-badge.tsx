import { medalRarityTone, type MedalRarity } from "@/lib/medal-rarity";
import { usableAssetUrl } from "@/components/medal-icon";

/**
 * 统一徽章：勋章墙 / 勋章殿堂 / 用户卡共用。
 *
 * 三层兜底解决「线上 `asset_ref` 全空 → 全站同一个 🏅」的老问题：
 *   1. 有 `asset_ref` → 画图（与头像框/道具图片同口径，存 URL 不存文件）；
 *   2. 无图 → **勋章名首字**（中文取首字、拉丁取首字母）走衬线字，像一枚印章；
 *      比共用 emoji 更有辨识度，且站长新增勋章零维护、无需 DB 列。
 *   3. 底色/边框/底边档条由 `data-tone` 派生（CSS `color-mix` 混合主题变量）——
 *      词表 tone 改了自动跟随，夜间主题不写死浅色。
 *
 * 颜色不是唯一载体：稀有度另有文字角标（`MedalRarityChip`）与悬停档案。
 */
export type MedalBadgeSize = "sm" | "md" | "lg";

function initialOf(name: string): string {
  const s = (name ?? "").trim();
  if (!s) return "🏅";
  return s.slice(0, 1).toUpperCase();
}

export function MedalBadge({
  name,
  assetRef,
  rarity,
  list,
  size = "md",
  worn = false,
  cap,
  wornLabel,
  className = "",
}: {
  name: string;
  assetRef?: string | null;
  rarity?: string | null;
  list: MedalRarity[];
  size?: MedalBadgeSize;
  worn?: boolean;
  /** 右上角库存角标（如「限 100」）；缺省不渲染 */
  cap?: string | null;
  /** 佩戴中时顶部小旗的文字（走字典，CSS 用 attr() 注入） */
  wornLabel?: string;
  className?: string;
}) {
  const tone = rarity ? medalRarityTone(list, rarity) : "";
  const url = usableAssetUrl(assetRef);
  return (
    <span
      className={`mb mb-${size}${worn ? " is-worn" : ""} ${className}`.trim()}
      data-tone={tone || undefined}
      data-worn-label={worn ? wornLabel : undefined}
      title={name}
    >
      {url ? (
        // eslint-disable-next-line @next/next/no-img-element
        <img src={url} alt={name} loading="lazy" />
      ) : (
        initialOf(name)
      )}
      {cap ? <span className="mb-cap">{cap}</span> : null}
    </span>
  );
}

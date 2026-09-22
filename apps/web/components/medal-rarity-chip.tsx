import {
  MEDAL_RARITY_TONES,
  medalRarityLabel,
  medalRarityTone,
  type MedalRarity,
} from "@/lib/medal-rarity";

/**
 * 稀有度角标：颜色由 globals.css 的 `.medal-rarity[data-tone=…]` 决定（走 token，随主题换肤）。
 *
 * 这里刻意**不用 `.sticker`** —— `.sticker:not([data-variant])` 有一条 unlayered 的兜底底色规则，
 * 会把 `bg-sky` 之类 utility 类的背景盖成纸白（实测角标全白、配色形同虚设）。
 */
export function MedalRarityChip({
  list,
  value,
  className = "",
}: {
  list: MedalRarity[];
  value?: string | null;
  className?: string;
}) {
  if (!value) return null;
  return (
    <span
      className={`medal-rarity ${className}`.trim()}
      data-tone={medalRarityTone(list, value)}
    >
      {medalRarityLabel(list, value)}
    </span>
  );
}

/** 词表编辑处的即时预览（用未保存的 label/tone 渲染） */
export function MedalRarityPreview({
  label,
  tone,
}: {
  label: string;
  tone: string;
}) {
  const safe = MEDAL_RARITY_TONES.includes(tone) ? tone : "sky";
  return (
    <span className="medal-rarity" data-tone={safe}>
      {label || "预览"}
    </span>
  );
}

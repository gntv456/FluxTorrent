/** 星卡卡面：卡面符号（星座符号）由卡 key 决定，色族由稀有度星数决定。
 *
 * 样式钩子是设计系统里的 .card[data-r]，这里把后端五档（R/SR/SSR/UR/LR）
 * 折叠成四个色族，避免为每档再抄一套渐变：星数 ≥5 走金，4 走紫，3 走蓝，
 * 其余中性。
 */

import { Badge } from "@/components/ui/badge";

/** 长 SVG path 折行拼接，避免单行超宽 */
const j = (...p: string[]) => p.join("");

export const SIGS: Record<string, string> = {
  key: j(
    '<circle cx="50" cy="30" r="9"/><circle cx="50" cy="30" r="2.6" f',
    'ill="currentColor" stroke="none"/><path d="M50 40V78M44 46h12M45',
    ' 54h10M46 62h8M47 70h6"/><path d="M50 12v4M50 47v3M30 30h6M64 30',
    'h6" opacity=".55"/>',
  ),
  beacon: j(
    '<path d="M38 78V38h24v40M31 78h38M40 38l10-14 10 14"/><path d="M',
    '50 24v-9M22 44l16 4M78 44l-16 4" opacity=".6"/><path d="M44 52h1',
    '2M44 62h12" opacity=".35"/>',
  ),
  compass: j(
    '<circle cx="50" cy="50" r="24"/><circle cx="50" cy="50" r="13" o',
    'pacity=".42"/><path d="M50 28v44M28 50h44" opacity=".5"/><path d',
    '="M50 50l13-13 5 5z" fill="currentColor" fill-opacity=".22"/><ci',
    'rcle cx="50" cy="50" r="2.4" fill="currentColor" stroke="none"/>',
  ),
  crown: j(
    '<path d="M32 62l-5-26 13 9 10-16 10 16 13-9-5 26z"/><path d="M30',
    ' 68h40"/><circle cx="50" cy="55" r="3"/>',
  ),
  firefly: j(
    '<path d="M50 30l10 11-10 25-10-25z"/><path d="M50 22v6M50 66v6M3',
    '4 44l7 3M66 44l-7 3" opacity=".6"/><circle cx="50" cy="47" r="3.',
    '2" fill="currentColor" stroke="none"/>',
  ),
  chart: j(
    '<circle cx="30" cy="34" r="2.4"/><circle cx="46" cy="26" r="2.4"',
    '/><circle cx="64" cy="38" r="2.4"/><circle cx="72" cy="60" r="2.',
    '4"/><circle cx="52" cy="68" r="2.4"/><circle cx="36" cy="58" r="',
    '2.4"/><path d="M30 34l16-8 18 12 8 22-20 8-16-10z" opacity=".45"',
    "/>",
  ),
  gear: j(
    '<circle cx="50" cy="50" r="19"/><circle cx="50" cy="50" r="7"/><',
    'path d="M50 26v-8M50 74v8M26 50h-8M74 50h8M32 32l-6-6M68 68l6 6M',
    '68 32l6-6M32 68l-6 6" opacity=".75"/>',
  ),
  crest: j(
    '<path d="M50 22l22 8v22c0 16-12 26-22 30-10-4-22-14-22-30V30z"/>',
    '<path d="M50 34v22M40 42h20" opacity=".6"/>',
  ),
  prism: j(
    '<path d="M50 26l19 44H31z"/><path d="M50 26v44" opacity=".45"/><',
    'path d="M50 44l-9 26M50 44l9 26" opacity=".7"/>',
  ),
  ruler: j(
    '<rect x="35" y="30" width="30" height="44" rx="3"/><path d="M35 ',
    '40h9M35 50h13M35 60h9M35 70h13" opacity=".85"/>',
  ),
  astrolabe: j(
    '<circle cx="50" cy="50" r="30"/><circle cx="50" cy="50" r="18" o',
    'pacity=".55"/><circle cx="50" cy="50" r="7"/><path d="M50 20v8M5',
    '0 72v8M20 50h8M72 50h8" opacity=".8"/><path d="M71 29l-5 5M34 66',
    'l-5 5M71 71l-5-5M34 34l-5-5" opacity=".5"/><circle cx="50" cy="5',
    '0" r="2.4" fill="currentColor" stroke="none"/>',
  ),
  seal: j(
    '<path d="M50 22l28 28-28 28-28-28z"/><path d="M50 34l16 16-16 16',
    '-16-16z" opacity=".45"/><circle cx="50" cy="50" r="3" fill="curr',
    'entColor" stroke="none"/><path d="M18 18l10 10M82 82l-10-10M82 1',
    '8l-10 10M18 82l10-10" opacity=".3"/>',
  ),
};

const SIG_KEYS = Object.keys(SIGS);

/** 卡 key → 符号：先按演示卡固定，再按 key 字符和散列到 12 个星座符号 */
export function sigFor(key: string, rarity?: string): string {
  const fixed: Record<string, string> = {
    "demo-r": "crest",
    "demo-sr": "compass",
    "demo-ssr": "key",
    "demo-ur": "prism",
    "demo-lr": "astrolabe",
  };
  if (fixed[key]) return fixed[key];
  if (rarity && (rarity === "UR" || rarity === "LR")) return "astrolabe";
  let h = 0;
  for (let i = 0; i < key.length; i += 1)
    h = (h * 31 + key.charCodeAt(i)) % 997;
  return SIG_KEYS[h % SIG_KEYS.length];
}

export function familyOf(rarity: string, stars: number): string {
  if (rarity === "UR" || rarity === "LR" || stars >= 5) return "SSR";
  if (stars >= 4) return "SR";
  if (stars >= 3) return "R";
  return "N";
}

export function CardFace({
  name,
  rarity,
  stars,
  tag,
  locked,
  dim,
  style,
}: {
  name: string;
  rarity: string;
  stars: number;
  tag?: string;
  locked?: boolean;
  dim?: boolean;
  style?: React.CSSProperties;
}) {
  const fam = familyOf(rarity, stars);
  return (
    <div
      className={`card${dim ? " dim" : ""}`}
      data-r={fam}
      style={style}
      aria-label={`${name} ${rarity}`}
    >
      <span className="card-gem" aria-hidden="true" />
      {tag && (
        <Badge tone="neutral" size="sm" className="card-tag">
          {tag}
        </Badge>
      )}
      <div className="card-art">
        <svg
          viewBox="0 0 100 100"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
          dangerouslySetInnerHTML={{
            __html: SIGS[sigFor(name + rarity, rarity)],
          }}
        />
      </div>
      <div className="card-plate">
        <span className="card-name">{locked ? "？？？" : name}</span>
        <div className="card-sub">
          <span className="card-stars" aria-hidden="true">
            {"★".repeat(Math.max(1, stars))}
          </span>
          <span className="card-lv">{locked ? "??" : rarity}</span>
        </div>
      </div>
    </div>
  );
}

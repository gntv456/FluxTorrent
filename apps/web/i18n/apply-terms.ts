/**
 * 术语覆盖层（0205 / 四审 L7）：把字典里写死的固有词（种子 / 魔力 / 保种…）
 * 按站长在后台注册的规则改成本站叫法。
 *
 * 语义与后端 `apps/api/src/terms.rs::apply` **逐条对齐**，两处出口必须同口径，
 * 否则同一句话在界面与提示语里会长得不一样：
 *   1) 长词优先（否则「种子」会抢在「种子文件」之前咬掉一半）；
 *   2) 单次正向扫描，替换出的文本不再参与匹配（互指规则不级联、不死循环）。
 * 排序按 **UTF-8 字节长度**降序，与 Rust 的 `str::len()` 一致。
 *
 * 生效点只有一处：`getDict()` 的出口（i18n/server.ts）。客户端经 LocaleProvider
 * 拿到的已是改写后的字典，所以 236 个 useI18n 消费点自动跟随。
 * **零规则时全程恒等返回**（不复制对象树），新装与未配置站点零成本。
 */

export interface TermRule {
  canonical: string;
  replacement: string;
}

const encoder = new TextEncoder();
const byteLen = (s: string): number => encoder.encode(s).length;

let sortedKey = "\u0000init";
let sortedCache: TermRule[] = [];

/** 排成替换用的顺序（长词优先，同长按词典序），并按规则集指纹缓存一份 */
export function sortRules(rules: TermRule[]): TermRule[] {
  const key = rules.map((r) => `${r.canonical}\u0001${r.replacement}`).join(
    "\u0002",
  );
  if (key === sortedKey) return sortedCache;
  const sorted = rules
    .filter((r) => r.canonical.length > 0 && r.replacement.length > 0)
    .slice()
    .sort(
      (a, b) =>
        byteLen(b.canonical) - byteLen(a.canonical) ||
        (a.canonical < b.canonical
          ? -1
          : a.canonical > b.canonical
            ? 1
            : 0),
    );
  sortedKey = key;
  sortedCache = sorted;
  return sorted;
}

/** 字幕区改名（0146／0208）的**保护词**：整体是另一个词（团队／角色），
 *  盲替换的副产物是生造词（字幕组→歌词组、认证字幕人→认证歌词人）。
 *  长词优先机制下用同词**等值规则**即可原样保留：长词先命中，且替换出的
 *  文本不再参与匹配。简体/繁体各一套（本站词典两种字形都有）。 */
export const SUBTITLE_PROTECTED_WORDS = ["字幕组", "字幕組", "字幕人"];

/** 站点设定的「字幕区显示名」→ 术语规则表（保护词在前，顺序无关，
 *  真正生效的是 sortRules 的字节长度降序） */
export function subtitleRules(label: string): TermRule[] {
  return [
    ...SUBTITLE_PROTECTED_WORDS.map((w) => ({
      canonical: w,
      replacement: w,
    })),
    { canonical: "字幕", replacement: label },
  ];
}

/** `{...}` 插值占位符的字符区间（`{` 到其后第一个 `}`）。
 *  与 Rust 侧 `placeholder_spans` 同语义：占位符整段是原子，绝不改写 ——
 *  规则原词若正好是 `magic` / `n`，撕开占位符会让 `fmt()` 取不到变量。 */
function placeholderSpans(text: string): [number, number][] {
  const spans: [number, number][] = [];
  let open = -1;
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i];
    if (c === "{") open = i;
    else if (c === "}" && open >= 0) {
      spans.push([open, i + 1]);
      open = -1;
    }
  }
  return spans;
}

const insideSpan = (
  spans: [number, number][],
  i: number,
): boolean => spans.some(([s, e]) => i > s && i < e);

/** 改写一段文本（与 Rust apply() 同语义） */
export function applyTermsText(text: string, rules: TermRule[]): string {
  if (!text || rules.length === 0) return text;
  const spans = placeholderSpans(text);
  let out = "";
  let i = 0;
  while (i < text.length) {
    const hit = insideSpan(spans, i)
      ? undefined
      : rules.find((r) => text.startsWith(r.canonical, i));
    if (hit) {
      out += hit.replacement;
      i += hit.canonical.length;
    } else {
      out += text[i];
      i += 1;
    }
  }
  return out;
}

function walk(v: unknown, rules: TermRule[]): unknown {
  if (typeof v === "string") return applyTermsText(v, rules);
  if (Array.isArray(v)) return v.map((x) => walk(x, rules));
  if (v && typeof v === "object") {
    const o: Record<string, unknown> = {};
    for (const [k, x] of Object.entries(v)) o[k] = walk(x, rules);
    return o;
  }
  return v;
}

/** 改写整本字典（只动字符串叶子，key 与结构不变，类型原样保留） */
export function applyTerms<T>(value: T, rules: TermRule[]): T {
  if (rules.length === 0) return value;
  return walk(value, rules) as T;
}

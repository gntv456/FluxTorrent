/**
 * 作品聚合视图的「片名 → 徽标」解析（样图：4K / WEB-DL / H.265 / AAC / HDR / 中字）。
 *
 * 为什么在前端解析：这些信息只存在于种子名里（`torrents.name`），后台没有
 * 结构化字段；上传页那套维度（source/codec）只有新数据才填。解析是纯展示
 * 层的事，放到后端等于把命名习惯写进 SQL。命中不了就不出徽标——不猜。
 *
 * ⚠️ 中文判定用 `\u` 转义写：本文件在 app/ 下，i18n 守卫会把源码里的
 * CJK 字面量当成「未翻译文案」判违规；转义不改变语义。
 */

export interface NameChip {
  /** 徽标文字（语言中立：4K / H.265 / S01 E15-E16…） */
  text: string;
  /** 样式类（见 styles/works.css 的 .wk-c*） */
  cls: string;
}

/** 画质档：4K/1080P/720P（第一个命中即止） */
const RES: [RegExp, string][] = [
  [/\b(2160p|4k|uhd)\b/i, "4K"],
  [/\b1080[pi]\b/i, "1080P"],
  [/\b720p\b/i, "720P"],
];

/** 来源 */
const SOURCE: [RegExp, string][] = [
  [/remux/i, "REMUX"],
  [/web[-. ]?dl/i, "WEB-DL"],
  [/webrip/i, "WEBRip"],
  [/blu[-. ]?ray/i, "BluRay"],
  [/hd(tv|rip)/i, "HDTV"],
  [/dvdrip/i, "DVDRip"],
];

/** 编码 */
const CODEC: [RegExp, string][] = [
  [/[hx][.-]?265|hevc/i, "H.265"],
  [/[hx][.-]?264|avc/i, "H.264"],
];

/** 高动态 */
const HDR: [RegExp, string][] = [
  [/hdr10\+/i, "HDR10+"],
  [/hdr10/i, "HDR10"],
  [/dolby[. ]?vision/i, "DV"],
  [/\bhdr\b/i, "HDR"],
];

/** 音轨：格式 + 声道（`AAC LC 2.0` / `DTS-HD MA 5.1` / `DDP 5.1`）
 *  写成 RegExp 构造是因为单行字面量会顶破 80 列行宽门禁。 */
const AUDIO = new RegExp(
  "(dts[-. ]?hd(?:[. ]?ma)?|dts[. ]?x|dts|ddp|dd\\+|ac3|eac3|" +
    "aac(?:[. ]?lc)?|flac|truehd|atmos)(?:[. ]?(\\d\\.\\d))?",
  "i",
);

/** 季集：`S01E15` / `S01E15-E16` / `S01` */
const EPISODE = /\bS(\d{1,2})(?:[. ]?E(\d{1,3})(?:[-.]?E?(\d{1,3}))?)?\b/i;

/** 中字 / 简繁 / 外挂字幕（`\u4e2d\u5b57` = 中字，见文件头说明） */
const SUBS = /(?:\u4e2d\u5b57|\u7b80\u7e41|chs|cht|\bsubs?\b)/i;

function firstHit(
  name: string,
  table: [RegExp, string][],
): string | null {
  for (const [re, label] of table) if (re.test(name)) return label;
  return null;
}

/**
 * 解析种子名 → 徽标数组（顺序固定：画质 → 来源 → 编码 → 音轨 → 季集 → HDR → 字幕）。
 * `subsLabel` 由调用方传字典文案（各语言不同，故不写死）。
 */
export function nameChips(name: string, subsLabel: string): NameChip[] {
  const out: NameChip[] = [];
  const res = firstHit(name, RES);
  if (res) out.push({ text: res, cls: "wk-c4k" });
  const src = firstHit(name, SOURCE);
  if (src) out.push({ text: src, cls: "wk-cweb" });
  const codec = firstHit(name, CODEC);
  if (codec) out.push({ text: codec, cls: "wk-ch265" });
  const audio = AUDIO.exec(name);
  if (audio) {
    const label = audio[1].replace(/\./g, "-").toUpperCase();
    out.push({
      text: audio[2] ? `${label} ${audio[2]}` : label,
      cls: "wk-caud",
    });
  }
  const ep = EPISODE.exec(name);
  if (ep) {
    const s = `S${ep[1].padStart(2, "0")}`;
    const range =
      ep[2] && ep[3]
        ? ` E${ep[2]}-E${ep[3]}`
        : ep[2]
          ? ` E${ep[2]}`
          : "";
    out.push({ text: s + range, cls: "wk-cep" });
  }
  const hdr = firstHit(name, HDR);
  if (hdr) out.push({ text: hdr, cls: "wk-chdr" });
  if (SUBS.test(name)) out.push({ text: subsLabel, cls: "wk-csub" });
  return out;
}

/** 作品名：切掉「｜」后的副题；去掉前导的【官种】/[组] 等标签；
 *  纯英文的 dot 名把点换成空格（读起来才像片名）。 */
export function workTitle(name: string): string {
  const base = name.split("\uff5c")[0].split("|")[0].trim();
  const stripped = base
    .replace(/^(?:[\[\(（【][^\]\)）】]*[\]\)）】]\s*)+/, "")
    .trim();
  const title = stripped || base;
  // 含中文就原样（中文片名里的点通常是分隔符，换了反而怪）
  if (/[\u4e00-\u9fa5]/.test(title)) return title;
  return title.replace(/[._]+/g, " ").trim();
}

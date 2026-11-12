/** 本文目录抽取（2026-11-12 帖子页三栏改版，机窝口径）。
 *
 *  与 forum-markdown 的 parseBlocks 同一遍块序做纯文本遍历——每个
 *  块消耗一个 `b${k}` 计数（围栏/引用/列表/段落同样耗号；种子卡/视频
 *  用 t/v 前缀不占 bK），标题块落在哪个 k 由这里推演，与渲染出的
 *  id=md-bK 一致。**两处遍历必须逐字同步**：改 parseBlocks 的块边界
 *  判定（围栏/引用/列表/段落吸收规则）须同步改这里，否则目录锚点错位。
 *  只收 h2/h3（h1 是帖标题，h4+ 太碎）；引用块内的 # 不是标题（引用
 *  整块吃掉，内部不解析，与渲染口径一致）。纯函数可在 RSC 服务端跑。 */

import {
  isFence,
  isHeading,
  isQuote,
  isUl,
  isOl,
  isHr,
  videoLine,
} from "@/components/forum-markdown";
import { torrentLine } from "@/lib/forum-torrent-line";

export interface TocEntry {
  id: string;
  text: string;
  level: number;
}

export function extractToc(src: string): TocEntry[] {
  const lines = src.replace(/\r\n?/g, "\n").split("\n");
  const out: TocEntry[] = [];
  let i = 0;
  let k = 0;
  while (i < lines.length) {
    const line = lines[i];
    const fence = line.match(/^\s*(```|~~~)(.*)$/);
    if (fence) {
      const marker = fence[1];
      i++;
      while (i < lines.length && !lines[i].trimStart().startsWith(marker))
        i++;
      i++;
      k++;
      continue;
    }
    if (line.trim() === "") {
      i++;
      continue;
    }
    if (torrentLine(line) !== null || videoLine(line)) {
      i++;
      continue;
    }
    if (isHr(line)) {
      k++;
      i++;
      continue;
    }
    const h = line.match(/^\s*(#{1,6})\s+(.*)$/);
    if (h) {
      const level = h[1].length;
      if (level === 2 || level === 3) {
        // renderInline 的纯文本近似：剥行内标记，保目录可读
        const text = h[2]
          .replace(/\*\*(.+?)\*\*/g, "$1")
          .replace(/\*(.+?)\*/g, "$1")
          .replace(/`(.+?)`/g, "$1")
          .replace(/\[(.+?)\]\([^)]*\)/g, "$1")
          .trim();
        if (text) out.push({ id: `md-b${k}`, text, level });
      }
      k++;
      i++;
      continue;
    }
    if (isQuote(line)) {
      while (i < lines.length && isQuote(lines[i])) i++;
      k++;
      continue;
    }
    if (isUl(line)) {
      while (i < lines.length && isUl(lines[i])) i++;
      k++;
      continue;
    }
    if (isOl(line)) {
      while (i < lines.length && isOl(lines[i])) i++;
      k++;
      continue;
    }
    // 段落：连续非空且非格式行合成一块（视频/种子行也是块级边界）
    i++;
    while (
      i < lines.length &&
      lines[i].trim() !== "" &&
      !isFence(lines[i]) &&
      !isHeading(lines[i]) &&
      !isQuote(lines[i]) &&
      !isUl(lines[i]) &&
      !isOl(lines[i]) &&
      !isHr(lines[i]) &&
      !videoLine(lines[i]) &&
      torrentLine(lines[i]) === null
    )
      i++;
    k++;
  }
  return out;
}

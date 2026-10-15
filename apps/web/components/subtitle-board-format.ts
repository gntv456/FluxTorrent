/** 字幕区展示工具（从 subtitle-board-table.tsx 拆出，300 行门禁）：
 *  语言字典缓存、旗帜/KB/相对时间格式化、规则文本加粗、附件下载。 */

import { api } from "@/lib/api-client";
import type { SubtitleRow } from "@/components/subtitle-board-shared";

/** 语言字典（GET /subtitles/langs；模块开着的站点启动时拉一次缓存于此） */
let langDict: { code: string; name: string; flag: string }[] | null = null;
export async function loadLangDict(): Promise<
  { code: string; name: string; flag: string }[]
> {
  if (langDict) return langDict;
  try {
    const rows = await api.get<
      { code: string; name: string; flag: string | null }[]
    >("/api/v1/subtitles/langs");
    langDict = rows.map((r) => ({
      code: r.code,
      name: r.name,
      flag: r.flag ?? "🌐",
    }));
  } catch {
    langDict = [];
  }
  return langDict;
}
/** 列表筛选下拉用（id 语义已废，直接回 code/name/flag） */
export const langOptions = loadLangDict;

export const LETTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ".split("");

export function langLabelOf(lang: string | null): string {
  if (!lang) return "Other";
  return langDict?.find((l) => l.code === lang)?.name ?? lang;
}
export function flagText(lang: string | null): string {
  if (!lang) return "🌐";
  return langDict?.find((l) => l.code === lang)?.flag ?? "🌐";
}
export function fmtKB(bytes: number): string {
  if (!bytes) return "—";
  const kb = bytes / 1024;
  return `${kb.toFixed(2)} KB`;
}
export function timeAgo(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const m = Math.floor(diff / 60000);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h${m % 60}m`;
  const d = Math.floor(h / 24);
  if (d < 30) return `${d}d${h % 24}h`;
  const mo = Math.floor(d / 30);
  return `${mo}mo${d % 30}d`;
}
/** 规则文本加粗关键部分（同步/标题/合集/Vobsub/proper） */
export function boldRule(r: string): string {
  return r
    .replace("字幕必须与视频文件同步", "<b>字幕必须与视频文件同步</b>")
    .replace("标题", "<b>标题</b>")
    .replace(/\&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/\*&lt;/g, "*<");
}

/** 行内 ⬇ 下载：本地附件端点直接回文件字节，带凭证拉取后触发保存；
 *  外部直链后端回 JSON，跳新页由其自行下载。桌面表格与移动卡片共用
 *  （原 25 行内联在 subtitle-board-table，M5.6 卡片化时上提）。
 *  只做 IO，不弹消息——成功/失败文案由调用方决定。 */
export async function downloadSubtitle(s: SubtitleRow): Promise<void> {
  const buf = await api.getBlob(`/api/v1/subtitles/${s.id}/download`);
  const ext = s.ext || "srt";
  const safe = s.title.replace(/[\\/:*?"<>|]/g, "_");
  const blob = new Blob([buf], { type: "application/octet-stream" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `${safe}.${ext}`;
  a.click();
  URL.revokeObjectURL(url);
}

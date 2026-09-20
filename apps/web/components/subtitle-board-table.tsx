"use client";

import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { SubtitleRow } from "@/components/subtitle-board-shared";

/** 字幕列表子面板（从 components/subtitle-board.tsx 按域拆出）：
 *  七列表格（语言/标题/添加时间/大小/点击/上传者/举报）+ 语言映射与
 *  展示工具（旗帜/KB/相对时间/规则加粗）。主组件与筛选/上传留在原文件。 */

/** NexusPHP 字幕语言表（value 与旧站 sel_lang 一致） */
export const LANGS: [string, string][] = [
  ["1", "Bulgarian"], ["2", "Croatian"], ["3", "Czech"], ["4", "Danish"],
  ["5", "Dutch"], ["6", "English"], ["7", "Estonian"], ["8", "Finnish"],
  ["9", "French"], ["10", "German"], ["11", "Greek"], ["12", "Hebrew"],
  ["13", "Hungarian"], ["14", "Italian"], ["15", "日本語"], ["16", "한국어"],
  ["17", "Norwegian"], ["18", "Other"], ["19", "Polish"], ["20", "Portuguese"],
  ["21", "Romanian"], ["22", "Russian"], ["23", "Serbian"], ["24", "Slovak"],
  ["25", "简体中文"], ["26", "Spanish"], ["27", "Swedish"], ["28", "繁體中文"],
  ["29", "Turkish"], ["30", "Slovenian"], ["31", "Thai"],
];
/** lang 存储值（chs/cht/eng…）→ 旧站数字 id 映射 */
export const LANG_CODE_TO_ID: Record<string, string> = {
  chs: "25", cht: "28", eng: "6", jpn: "15", kor: "16", other: "18",
};
const LANG_ID_TO_LABEL = (id: string) => LANGS.find(([v]) => v === id)?.[1] ?? id;

export const LETTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ".split("");

export function langLabelOf(lang: string | null): string {
  if (!lang) return "Other";
  return LANG_ID_TO_LABEL(LANG_CODE_TO_ID[lang] ?? "18");
}
function flagText(lang: string | null): string {
  const label = langLabelOf(lang);
  if (label === "简体中文") return "🇨🇳";
  if (label === "繁體中文") return "🇹🇼";
  if (label === "English") return "🇬🇧";
  if (label === "日本語") return "🇯🇵";
  if (label === "한국어") return "🇰🇷";
  return "🌐";
}
export function fmtKB(bytes: number): string {
  if (!bytes) return "—";
  const kb = bytes / 1024;
  return `${kb.toFixed(2)} KB`;
}
export function timeAgo(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const m = Math.floor(diff / 60000);
  if (m < 60) return `${m}分钟`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}时${m % 60}分`;
  const d = Math.floor(h / 24);
  if (d < 30) return `${d}天${h % 24}时`;
  const mo = Math.floor(d / 30);
  return `${mo}月${d % 30}天`;
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

/** 字幕列表（七列）：本地附件走 BLOB 下载，外部直链跳新页；举报走 prompt */
export function SubtitleListTable({
  rows,
  onMsg,
  onReload,
}: {
  rows: SubtitleRow[];
  onMsg: (m: string) => void;
  onReload: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.subtitles;
  return (
    <div className="baozi-wide-table-scroll subtitles-table-scroll" role="region">
      <table className="nexus-table subtitles-list-table">
        <tbody>
          <tr>
            <td className="colhead">{t.colLang}</td>
            <td className="colhead text-center">{t.colTitle}</td>
            <td className="colhead text-center">{t.colTime}</td>
            <td className="colhead text-center">{t.colSize}</td>
            <td className="colhead text-center">{t.colHits}</td>
            <td className="colhead text-center">{t.colUploader}</td>
            <td className="colhead text-center">{t.colReport}</td>
          </tr>
          {rows.map((s) => (
            <tr key={s.id}>
              <td className="text-center" title={langLabelOf(s.lang)}>
                <span className="subtitles-flag" aria-hidden="true">
                  {flagText(s.lang)}
                </span>
              </td>
              <td>
                <a
                  href={`/api/v1/subtitles/${s.id}/download`}
                  className="font-bold"
                  target="_blank"
                  rel="noreferrer"
                  onClick={async (e) => {
                    // 本地附件（attach://sha）端点直接回文件字节：带 Bearer 拉取后
                    // 触发浏览器保存；外部直链（http ref）后端回 JSON，跳新页由其自行下载
                    e.preventDefault();
                    try {
                      const buf = await api.getBlob(
                        `/api/v1/subtitles/${s.id}/download`,
                      );
                      const ctype = "application/octet-stream";
                      const name = `${s.title.replace(/[\\/:*?"<>|]/g, "_")}.srt`;
                      const blob = new Blob([buf], { type: ctype });
                      const url = URL.createObjectURL(blob);
                      const a = document.createElement("a");
                      a.href = url;
                      a.download = name;
                      a.click();
                      URL.revokeObjectURL(url);
                      onMsg(`字幕「${s.title}」已开始下载`);
                      onReload();
                    } catch (err) {
                      onMsg(err instanceof Error ? err.message : "下载失败");
                    }
                  }}
                >
                  {s.title}
                </a>
              </td>
              <td className="nowrap text-center" title={new Date(s.created_at).toLocaleString("zh-CN")}>
                {timeAgo(s.created_at)}
              </td>
              <td className="num text-center">{fmtKB(s.size ?? 0)}</td>
              <td className="num text-center">{s.downloads}</td>
              <td className="text-center">
                <span className="nowrap">{s.username ?? t.noAccount}</span>
              </td>
              <td className="text-center">
                <button
                  type="button"
                  className="subtitles-report"
                  title={t.reportTitle}
                  aria-label={t.reportTitle}
                  onClick={async () => {
                    const reason = window.prompt(t.reportTitle);
                    if (!reason?.trim()) return;
                    try {
                      await api.post("/api/v1/reports", {
                        ref_type: "subtitle",
                        ref_id: s.id,
                        reason: reason.trim(),
                      });
                      onMsg(t.reportOk ?? "举报已提交，感谢反馈");
                    } catch (e) {
                      onMsg(e instanceof Error ? e.message : (t.reportFail ?? "举报失败"));
                    }
                  }}
                >
                  ⚑
                </button>
              </td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={7} className="py-8 text-center text-sub">
                {t.empty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}

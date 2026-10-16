"use client";

import { useI18n } from "@/i18n/client";

/** 编辑弹层·MediaInfo + NFO 两个文本域（批 B）。
 *
 *  为什么单独成文件：`torrent-manage-fields.tsx` 的行数基线顶在 305
 *  （line_limit_guard 对存量超限文件「只许瘦不许胖」），在它内部加 NFO
 *  必然把红账推高。这里搬出 MediaInfo 块并带上新的 NFO 块，
 *  fields.tsx 反而变短。
 *
 *  NFO 的三态口径（后端 `torrents::TorrentEdit`）：不提交=不动、
 *  空串=清空、非空=覆盖。表单里刻意不做「预填当前 NFO」——聚合口
 *  不下发 nfo 全文（预填要新增读口，代价大），因此这里的语义是
 *  **留空即保持原值**，hint 文案就这么写，别让用户以为留空会清空。 */

const MONO_AREA =
  "rounded-[var(--r-sm)] border border-line px-2 py-1 font-mono text-xs";

type Dict = ReturnType<typeof useI18n>["dict"];

export function MetaNfoFields({
  dict,
  metaSrc,
  fMediainfo,
  setFMediainfo,
  fNfo,
  setFNfo,
}: {
  dict: Dict;
  metaSrc: string[];
  fMediainfo: string;
  setFMediainfo: (v: string) => void;
  fNfo: string;
  setFNfo: (v: string) => void;
}) {
  const u = dict.upload;
  const t = dict.torrentManage2;
  return (
    <>
      {/* MediaInfo（0209 P2-14）：与发布页同口径，mediainfo 源关掉即隐藏 */}
      {metaSrc.includes("mediainfo") && (
        <label className="flex flex-col gap-1 text-xs">
          {u.mediainfoLabel}
          <textarea
            value={fMediainfo}
            rows={4}
            onChange={(e) => setFMediainfo(e.target.value)}
            className={MONO_AREA}
          />
        </label>
      )}
      {/* NFO（批 B）：此前只有发布口能写，传错只能删重发 */}
      <label className="flex flex-col gap-1 text-xs">
        {t.nfoTitle}
        <textarea
          value={fNfo}
          rows={4}
          onChange={(e) => setFNfo(e.target.value)}
          className={MONO_AREA}
        />
        <span className="text-[11px] text-sub">{t.nfoKeepHint}</span>
      </label>
    </>
  );
}

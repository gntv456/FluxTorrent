"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { uploadAttachment } from "@/components/subtitle-attachment";
import { Modal } from "@/components/modal";
import {
  SubtitleUploadFields,
  type SubUploadState,
} from "@/components/subtitle-upload-fields";

/** 字幕上传：按钮弹窗式（2026-11-15——原先整张表单平铺在字幕区页面上，
 *  十来行输入把规则卡和筛选条挤开，观感差；改「上传字幕」按钮 + Modal
 *  弹窗发布，Modal 是站内标准件（详情页编辑/购买置顶免费同款）。
 *  字段表格拆 subtitle-upload-fields（300 行门禁），本文件留状态+提交。 */

const FRESH: SubUploadState = {
  title: "",
  torrentId: "",
  lang: "",
  file: null,
  fps: "",
  source: "",
  author: "",
  anon: false,
  ai: false,
  aiProof: false,
  proofreader: "",
};

export function SubtitleUploadForm({
  kind,
  langs,
  fixedTorrentId,
  onMsg,
  onUploaded,
}: {
  kind: "subtitle" | "lyric";
  langs: { code: string; name: string; flag: string }[];
  fixedTorrentId?: number;
  onMsg: (m: string) => void;
  onUploaded: () => void;
}) {
  const { dict, currency } = useI18n();
  const t = dict.subtitles;
  const [open, setOpen] = useState(false);
  const [s, setS] = useState<SubUploadState>({
    ...FRESH,
    torrentId: fixedTorrentId ? String(fixedTorrentId) : "",
  });
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const set = (patch: Partial<SubUploadState>) =>
    setS((v) => ({ ...v, ...patch }));

  function resetAll() {
    setS({ ...FRESH, torrentId: fixedTorrentId ? String(fixedTorrentId) : "" });
    setMsg(null);
  }

  function close() {
    setOpen(false);
    resetAll();
  }

  async function upload() {
    if (!s.title.trim() || !s.lang || !s.file) return;
    if (s.file.size > 8 * 1024 * 1024) {
      setMsg(t.tooLarge ?? "file too large");
      return;
    }
    setBusy(true);
    setMsg(null);
    try {
      const sha = await uploadAttachment(s.file);
      await api.post("/api/v1/subtitles", {
        torrent_id: s.torrentId ? Number(s.torrentId) : 0,
        title: s.title,
        lang: s.lang,
        file_sha: sha,
        fps: s.fps ? Number(s.fps) : undefined,
        source: s.source || undefined,
        author_name: s.author || undefined,
        anon: s.anon,
        machine_translated: s.ai,
        proofreader: s.ai && s.aiProof
          ? s.proofreader || (t.proofreadMarked ?? "proofread")
          : undefined,
      });
      // 成功即收摊：关弹窗 + 清表单；提示交给页面级 onMsg（列表上方）
      onMsg(t.reward.replace("{magic}", currency));
      close();
      onUploaded();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      {/* 触发按钮：与标题条「总上传量」并排 */}
      <button
        type="button"
        className="btn"
        onClick={() => {
          resetAll();
          setOpen(true);
        }}
      >
        {t.uploadTitle}
      </button>
      <Modal open={open} onClose={close} title={t.uploadTitle} size="lg">
        <form
          className="subtitles-upload-form"
          onSubmit={(e) => {
            e.preventDefault();
            void upload();
          }}
        >
          <SubtitleUploadFields
            kind={kind}
            langs={langs}
            fixedTorrentId={fixedTorrentId}
            s={s}
            set={set}
            busy={busy}
          />
          {msg && <p className="subtitles-msg">{msg}</p>}
        </form>
      </Modal>
    </>
  );
}

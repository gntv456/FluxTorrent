"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { uploadAttachment } from "@/components/subtitle-attachment";
import { LangSelect } from "@/components/subtitle-lang-select";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";

/** 字幕发布弹窗（2026-11-13 从 torrent-subtitles 拆出，发布改弹窗批）：
 *  表单进 ConfirmDialog children 槽——聚焦陷阱/Esc/遮罩/body 锁滚全复用；
 *  确认钮即「上传文件」。三字段（文件/标题/语言）缺一不可：缺时不关
 *  弹窗，弹窗内 msg 行提示（面板侧提示被遮罩挡住）。
 *
 *  状态自持：表单字段与 busy 都在本组件，父级只管 open 与成功后刷新；
 *  成功后自关并清表单，失败留在弹窗里显示错误。 */

export function SubtitleUploadDialog({
  torrentId,
  open,
  onClose,
  onDone,
}: {
  torrentId: number;
  open: boolean;
  onClose: () => void;
  /** 发布成功回调（父级刷新列表） */
  onDone: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.subtitles;
  const [fTitle, setFTitle] = useState("");
  const [fLang, setFLang] = useState("");
  const [fFile, setFFile] = useState<File | null>(null);
  const [fAnon, setFAnon] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function upload() {
    if (!fTitle.trim() || !fLang || !fFile) return;
    setBusy(true);
    setMsg(null);
    try {
      const sha = await uploadAttachment(fFile);
      await api.post("/api/v1/subtitles", {
        torrent_id: torrentId,
        title: fTitle,
        lang: fLang,
        file_sha: sha,
        anon: fAnon,
      });
      setFTitle("");
      setFFile(null);
      onClose();
      onDone();
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <ConfirmDialog
      open={open}
      onClose={onClose}
      onConfirm={() => {
        if (!fFile || !fTitle.trim() || !fLang) {
          setMsg(t.requiredNote);
          return;
        }
        void upload();
      }}
      title={t.uploadTitle}
      confirmLabel={busy ? t.uploadBusy : t.uploadBtn}
      busy={busy}
    >
      <div className="td-sub-form">
        <label className="td-sub-form__row">
          <span>{t.file}</span>
          <input
            type="file"
            accept=".srt,.ass,.ssa,.sup,.idx,.sub,.cue,.zip,.rar,.7z,.lrc"
            onChange={(e) => setFFile(e.target.files?.[0] ?? null)}
            aria-label={t.file}
          />
        </label>
        <label className="td-sub-form__row">
          <span>{t.titleLabel}</span>
          <input
            type="text"
            placeholder={t.titlePlaceholder}
            value={fTitle}
            onChange={(e) => setFTitle(e.target.value)}
          />
        </label>
        <div className="td-sub-form__row">
          <span>{t.lang}</span>
          <LangSelect value={fLang} onChange={setFLang} />
        </div>
        <label className="td-sub-form__row td-sub-form__row--check">
          <input
            type="checkbox"
            checked={fAnon}
            onChange={(e) => setFAnon(e.target.checked)}
          />
          <span>{t.anonLabel ?? "Anonymous"}</span>
        </label>
        {msg && (
          <p className="td-sub-form__msg" role="status">
            {msg}
          </p>
        )}
      </div>
    </ConfirmDialog>
  );
}

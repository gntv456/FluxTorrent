"use client";

/** 发布表单·NFO / 抓轨日志 / 图床附件（从 upload-form-files.tsx 按域拆出，
 *  300 行门禁）。图床上传成功后把 [img]URL[/img] 追加进简介（0100 口径）。 */

import { useState } from "react";
import type { RefObject } from "react";
import { attachUpload } from "@/components/upload-attach-io";
import { useI18n } from "@/i18n/client";
import { FormRow } from "@/components/upload-form-parts";
import { UploadRipLogs } from "@/components/upload-form-riplogs";

/** 附件允许的类型（与原表单一致；拆行守 80 列） */
const ATTACH_ACCEPT =
  "image/png,image/jpeg,image/gif,image/webp,image/avif," +
  "application/pdf,text/plain";

export function UploadNfoAttach({
  nfoRef,
  setDescr,
}: {
  nfoRef: RefObject<HTMLInputElement | null>;
  setDescr: (v: string | ((prev: string) => string)) => void;
}) {
  const { dict } = useI18n();
  const [nfoName, setNfoName] = useState("");
  const [attachBusy, setAttachBusy] = useState(false);
  const [attachMsg, setAttachMsg] = useState<string | null>(null);

  async function uploadAttachment(file: File) {
    setAttachBusy(true);
    setAttachMsg(null);
    try {
      const r = await attachUpload(file, dict, setDescr);
      setAttachMsg(r.msg);
    } catch (e) {
      setAttachMsg(
        e instanceof Error && e.message ? e.message : dict.upload.attachFail,
      );
    } finally {
      setAttachBusy(false);
    }
  }

  return (
    <>
      <FormRow label={dict.upload.nfo}>
        <div className="flex flex-col gap-1">
          <label className="uf-drop uf-drop--optional">
            <input
              ref={nfoRef}
              name="nfo"
              type="file"
              accept=".nfo,text/plain"
              className="sr-only"
              onChange={(e) => setNfoName(e.target.files?.[0]?.name ?? "")}
            />
            {nfoName ? `📄 ${nfoName}` : dict.upload.nfoHint}
          </label>
        </div>
      </FormRow>
      {/* 抓轨日志（0312）：站点未开检查时后端忽略这些 part */}
      <UploadRipLogs />
      <FormRow label={dict.upload.attachLabel}>
        <div className="flex flex-col gap-1">
          <label className="uf-drop uf-drop--optional">
            <input
              type="file"
              name="attach"
              className="sr-only"
              accept={ATTACH_ACCEPT}
              disabled={attachBusy}
              onChange={(e) => {
                const f = e.target.files?.[0];
                if (f) void uploadAttachment(f);
                e.currentTarget.value = "";
              }}
            />
            {attachBusy
              ? dict.upload.attachUploading
              : dict.upload.attachHint}
          </label>
          {attachMsg && <p className="text-xs text-sky-deep">{attachMsg}</p>}
        </div>
      </FormRow>
    </>
  );
}

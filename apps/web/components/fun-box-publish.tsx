"use client";

import { useI18n } from "@/i18n/client";

/** 趣味盒前台·发布表单（从 components/fun-box.tsx 按域拆出）：
 *  标题 + 正文（24h 冷却提示）。 */

export function FunPublishForm({
  busy,
  pTitle,
  setPTitle,
  pBody,
  setPBody,
  publish,
}: {
  busy: boolean;
  pTitle: string;
  setPTitle: (v: string) => void;
  pBody: string;
  setPBody: (v: string) => void;
  publish: () => void;
}) {
  const t = useI18n().dict.funbox;
  return (
    <div className="funbox__publish cmgmt-form">
      <label>
        {t.fldTitle}
        <input
          value={pTitle}
          onChange={(e) => setPTitle(e.target.value)}
          maxLength={255}
        />
      </label>
      <label>
        {t.fldBody}
        <textarea
          rows={4}
          value={pBody}
          onChange={(e) => setPBody(e.target.value)}
        />
      </label>
      <p className="funbox__note">{t.cooldownNote}</p>
      <button
        type="button"
        className="baozi-button"
        disabled={busy || !pTitle.trim()}
        onClick={publish}
      >
        {t.submit}
      </button>
    </div>
  );
}

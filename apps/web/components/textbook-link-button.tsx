"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

/** 课本 ↔ 种子关联（textbooks/link）：把已发布种子挂到课本卡片 */
export function TextbookLinkButton({ textbookId }: { textbookId: number }) {
  const { dict } = useI18n();
  const t = dict.textbookLink2 ?? {
    link: "关联种子",
    linkPrompt: "填入种子 ID",
    linkInvalid: "请输入正整数种子 ID",
    linkedOk: "已关联",
  };
  const [busy, setBusy] = useState(false);

  return (
    <button
      type="button"
      disabled={busy}
      className="text-xs font-bold text-sky-deep disabled:opacity-50"
      onClick={async () => {
        const v = prompt(t.linkPrompt);
        const tid = v ? parseInt(v, 10) : NaN;
        if (!Number.isFinite(tid) || tid <= 0) {
          if (v !== null) alert(t.linkInvalid);
          return;
        }
        setBusy(true);
        try {
          await api.post("/api/v1/textbooks/link", {
            torrent_id: tid,
            textbook_id: textbookId,
          });
          window.location.reload();
        } catch (e) {
          alert(apiErrorMessage(dict, e));
        } finally {
          setBusy(false);
        }
      }}
    >
      + {t.link}
    </button>
  );
}

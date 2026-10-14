"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";
import { toast } from "@/components/ui/toast";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";

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
  // P2 清偿：种子 ID 采集（window.prompt 退役 → 输入槽）
  const ask = useConfirm();

  async function link(raw: string) {
    const tid = raw ? parseInt(raw, 10) : NaN;
    if (!Number.isFinite(tid) || tid <= 0) {
      // W-24 反馈协议：校验失败 toast.warn（不抢焦点，可继续输入）
      toast.warn(t.linkInvalid);
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
      // W-24：失败 toast 带 request_id 芯片（可复制报障）
      toast.error(apiErrorMessage(dict, e), {
        requestId: e instanceof ApiError ? e.requestId : undefined,
      });
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <button
        type="button"
        disabled={busy}
        className="text-xs font-bold text-sky-deep disabled:opacity-50"
        onClick={() =>
          ask.open({
            title: t.link,
            label: t.linkPrompt,
            busy,
            run: (raw) => void link(raw),
          })
        }
      >
        + {t.link}
      </button>
      <AskDialog {...ask} />
    </>
  );
}

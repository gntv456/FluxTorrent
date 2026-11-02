"use client";

import { useCallback, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { toast } from "@/components/ui/toast";
import type { MessageRow } from "@/components/message-center-table";

/** 写信状态机（从 components/message-center.tsx 按域拆出，300 行门禁）。
 *
 *  回复/转发/新写三态共用一套字段与一个提交口；「Re:/Fw: 前缀不重复叠加」
 *  与「发完切到已发送」都在这里，MessageCenter 只拿状态与两个动作。
 *  2026-11-02：随 /messages 手机端改造一并拆出（那边要加真分页，
 *  原地扩就把 300 行顶破了）。 */
export function useMessageCompose(onSent: () => void) {
  const { dict } = useI18n();
  const t = dict.messages;
  const [composing, setComposing] = useState(false);
  const [replyTo, setReplyTo] = useState<number | null>(null);
  const [forwardOf, setForwardOf] = useState<number | null>(null);
  const [to, setTo] = useState("");
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);

  const open = useCallback(() => {
    setReplyTo(null);
    setForwardOf(null);
    setTo("");
    setSubject("");
    setBody("");
    setComposing((v) => !v);
  }, []);

  /** 打开详情即置已读（短路同条重复打开） */
  const startFrom = useCallback(
    (m: MessageRow, mode: "reply" | "forward") => {
      setReplyTo(mode === "reply" ? m.id : null);
      setForwardOf(mode === "forward" ? m.id : null);
      setTo(mode === "reply" ? (m.counterpart ?? "") : "");
      const pfx = mode === "reply" ? "Re" : "Fw";
      setSubject(
        new RegExp(`^${pfx}:`, "i").test(m.subject)
          ? m.subject
          : `${pfx}: ${m.subject}`,
      );
      setBody("");
      setComposing(true);
    },
    [],
  );

  const send = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setBusy(true);
      try {
        await api.post("/api/v1/messages", {
          to: to.trim(),
          subject: subject.trim(),
          body: body.trim(),
          ...(replyTo ? { reply_to: replyTo } : {}),
          ...(forwardOf ? { forward_of: forwardOf } : {}),
        });
        toast.success(t.sentOk);
        setTo("");
        setSubject("");
        setBody("");
        setReplyTo(null);
        setForwardOf(null);
        setComposing(false);
        onSent();
      } catch (e2) {
        const ae = e2 instanceof ApiError ? e2 : undefined;
        toast.error(
          ae ? apiErrorMessage(dict, ae) : dict.common.networkError,
          { requestId: ae?.requestId },
        );
      } finally {
        setBusy(false);
      }
    },
    [body, dict, forwardOf, onSent, replyTo, subject, t.sentOk, to],
  );

  return {
    composing,
    setComposing,
    replyTo,
    forwardOf,
    to,
    setTo,
    subject,
    setSubject,
    body,
    setBody,
    busy,
    open,
    startFrom,
    send,
  };
}

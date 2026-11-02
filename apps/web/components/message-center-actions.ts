"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { toast } from "@/components/ui/toast";

/** 消息中心的写动作（从 components/message-center.tsx 按域拆出，300 行门禁）。
 *
 *  批量已读/删除/移动 + 自建文件夹的建改删，统一 toast 反馈；批量删除的
 *  二次确认留在渲染区（ConfirmDialog danger），这里只执行。 */
export function useMessageActions({
  load,
  selected,
  setAskBulkDelete,
  onBoxSaved,
}: {
  /** 列表重载（写操作后刷新） */
  load: () => void;
  /** 当前选中的消息 id */
  selected: number[];
  /** 批量删除的二次确认开关（交给渲染区） */
  setAskBulkDelete: (v: boolean) => void;
  /** 文件夹保存成功后的收尾（清名 + 收起面板） */
  onBoxSaved: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.messages;
  const [busy, setBusy] = useState(false);

  /** 写操作失败统一出口：字典文案优先 + request_id 芯片 */
  function toastError(e: unknown) {
    const ae = e instanceof ApiError ? e : undefined;
    toast.error(
      ae ? apiErrorMessage(dict, ae) : dict.common.networkError,
      { requestId: ae?.requestId },
    );
  }

  async function bulk(
    action: "markread" | "delete" | "move",
    target?: number | null,
  ) {
    if (selected.length === 0) return;
    // ZT81：删除二次确认在渲染区 ConfirmDialog（W-23），此处只执行
    setBusy(true);
    const path =
      `/api/v1/messages/${action === "markread" ? "markread" : action}`;
    try {
      await api.post(path, {
        ids: selected,
        ...(action === "move" ? { folder: target ?? null } : {}),
      });
      load();
      toast.success(t.bulkDone.replace("{n}", String(selected.length)));
    } catch (e) {
      toastError(e);
    } finally {
      setBusy(false);
    }
  }

  async function upsertBox(id: number | null, name: string) {
    setBusy(true);
    try {
      await api.post("/api/v1/messages/boxes", { id, name });
      onBoxSaved();
      load();
      toast.success(id ? t.boxRenamed : t.boxCreated);
    } catch (e) {
      toastError(e);
    } finally {
      setBusy(false);
    }
  }

  return { busy, bulk, upsertBox, toastError };
}

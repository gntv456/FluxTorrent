"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { PromoKindResp } from "./admin-promo-shared";

/** 促销档位/价目面板共用的取数与状态（0213）。
 *  由 AdminPromoKinds 持有，向下传给 AdminPromoTiers（避免双请求/消息割裂）。 */
export function usePromoKindData() {
  const { dict } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  const [data, setData] = useState<PromoKindResp | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const reload = useCallback(async () => {
    try {
      setData(await api.get<PromoKindResp>("/api/v1/admin/promo-kinds"));
    } catch {
      setData(null);
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  /** 统一错误展示（错误码字典优先，回落通用失败文案） */
  const fail = useCallback(
    (e: unknown) => {
      setMsg(
        e instanceof ApiError
          ? (dict.errors[e.code] ?? e.message)
          : a.actionFailed,
      );
    },
    [dict, a.actionFailed],
  );

  /** 包一层 busy 闸门：请求期间按钮禁用，失败走 fail，成功走 after */
  const run = useCallback(
    async (fn: () => Promise<void>, after?: string) => {
      setBusy(true);
      try {
        await fn();
        if (after) setMsg(after);
        await reload();
      } catch (e) {
        fail(e);
      } finally {
        setBusy(false);
      }
    },
    [fail, reload],
  );

  return { data, msg, setMsg, busy, reload, fail, run };
}

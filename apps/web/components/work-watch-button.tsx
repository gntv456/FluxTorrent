"use client";

/** 作品页「订阅新作」按钮（0424 P1-2）：POST /works/{key}/watch 把作品
 *  代表种名写进愿望单，复用其通知全链（24h 节流/聚合站内信+push）。 */

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

const BTN =
  "mt-2 w-full rounded-[var(--r-sm)] border border-line px-3 py-2 " +
  "text-xs font-bold text-ink transition hover:border-" +
  "[var(--brand-orange)] hover:text-[var(--brand-orange)] " +
  "disabled:opacity-60";

export function WorkWatchButton({
  workKey,
  initialWatching = false,
}: {
  workKey: string;
  /** 服务端回读：作品已在愿望单里（重载后不该退回「订阅新作」） */
  initialWatching?: boolean;
}) {
  const { dict } = useI18n();
  const t = dict.works.watch;
  const [state, setState] = useState<"idle" | "busy" | "done" | "fail">(
    initialWatching ? "done" : "idle",
  );
  // 业务拒绝（愿望单满 / 模块关闭）要原样透出后端中文，否则界面只剩
  // 「订阅失败，请重试」——用户会无限重试一个永远不会成功的动作。
  const [errMsg, setErrMsg] = useState<string | null>(null);
  async function watch() {
    setState("busy");
    setErrMsg(null);
    try {
      await api.post(`/api/v1/works/${workKey}/watch`);
      setState("done");
    } catch (e) {
      setErrMsg(e instanceof ApiError ? e.message : null);
      setState("fail");
    }
  }
  return (
    <button
      type="button"
      onClick={watch}
      disabled={state === "busy" || state === "done"}
      className={BTN}
    >
      {state === "done"
        ? t.done
        : state === "fail"
          ? (errMsg ?? t.fail)
          : state === "busy"
            ? t.busy
            : t.add}
    </button>
  );
}

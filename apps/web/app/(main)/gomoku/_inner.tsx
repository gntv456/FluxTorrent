"use client";

;

import { useEffect, useState } from "react";
import { api, ApiError, hasSessionCookie } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { GomokuBoard } from "@/components/plugins";

/** 五子棋（wuziqi 插件复刻）：建房/加入/在线对弈 */
export default function GomokuPage() {
  const { dict } = useI18n();
  const [me, setMe] = useState<number | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    if (!hasSessionCookie()) return;
    api
      .get<{ id: number }>("/api/v1/me")
      .then((r) => setMe(r.id))
      .catch((e) => setErr(e instanceof ApiError ? e.message : dict.common.loadFailed));
  }, [dict]);

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.gomoku.title}</h1>
      {err ? (
        <p className="baozi-panel p-4 text-sm text-sub">{err}</p>
      ) : me === null ? (
        <p className="baozi-panel p-4 text-sm text-sub">{dict.common.pleaseLogin}</p>
      ) : (
        <GomokuBoard meId={me} />
      )}
    </div>
  );
}

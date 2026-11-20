"use client";

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
      .catch((e) =>
        setErr(e instanceof ApiError ? e.message : dict.common.loadFailed),
      );
  }, [dict]);

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.gomoku.title}</h1>
      {err ? (
        <p className="brand-panel p-4 text-sm text-sub">{err}</p>
      ) : me === null ? (
        <div className="brand-panel flex flex-col items-start gap-2
                                   p-4 text-sm text-sub">
          <span>{dict.common.pleaseLogin}</span>
          <a
            href="/login?next=%2Fgomoku"
            className="min-h-11 rounded-full bg-sky px-4 font-bold
                                       text-[var(--text-on-accent)]"
          >
            {dict.review.loginAndContinue}
          </a>
        </div>
      ) : (
        <GomokuBoard meId={me} />
      )}
    </div>
  );
}

"use client";

;

import { useEffect, useState } from "react";
import { api, ApiError, hasSessionCookie } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { FrameShop } from "@/components/plugins";

/** 头像挂件（avatar_frame 插件复刻）：佩戴/更换头像挂件，付费件扣魔力 */
export default function AvatarFramesPage() {
  const { dict, currency } = useI18n();
  const [me, setMe] = useState<{ id: number; avatar_frame_id: number | null } | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    if (!hasSessionCookie()) return;
    api
      .get<{ id: number; avatar_frame_id: number | null }>("/api/v1/me")
      .then(setMe)
      .catch((e) => setErr(e instanceof ApiError ? e.message : dict.common.loadFailed));
  }, [dict]);

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.frames.title}</h1>
      <p className="text-sm text-sub">{dict.frames.subtitle.replace("{magic}", currency)}</p>
      {err ? (
        <p className="baozi-panel p-4 text-sm text-sub">{err}</p>
      ) : (
        <FrameShop
          current={me?.avatar_frame_id ?? null}
          onChange={(id) => setMe((m) => (m ? { ...m, avatar_frame_id: id } : m))}
        />
      )}
    </div>
  );
}

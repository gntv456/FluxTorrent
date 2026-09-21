"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** hxpt 插件移植前台（从 components/plugins.tsx 按域拆出）：
 *  头像挂件 FrameShop——挂件卡片网格 + 佩戴/取下。 */

export interface Frame {
  id: number;
  name: string;
  css: string;
  image_url?: string | null;
  price: number;
}

// 挂件卡片网格（自适应列）
const FRAME_GRID = "grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-3";
// 卡片容器 / 卡片上的两种操作按钮
const FRAME_CARD = "baozi-panel flex flex-col items-center gap-2 p-4";
const FRAME_BTN_OFF =
  "min-h-[32px] rounded-full border border-line px-3 " +
  "text-xs font-bold disabled:opacity-50";
const FRAME_BTN_ON =
  "min-h-[32px] rounded-full bg-sky px-3 text-xs " +
  "font-bold text-white disabled:opacity-50";
// 挂件预览图（absolute 铺满卡片图标）
const FRAME_IMG =
  "pointer-events-none absolute inset-0 z-10 h-full " +
  "w-full select-none object-fill";

export function FrameShop({
  current,
  onChange,
}: {
  current: number | null;
  onChange?: (id: number | null) => void;
}) {
  const { dict } = useI18n();
  const t = dict.frames;
  const [frames, setFrames] = useState<Frame[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api
      .get<Frame[]>("/api/v1/avatar-frames")
      .then(setFrames)
      .catch(() => setFrames([]));
  }, []);

  async function equip(id: number | null) {
    setBusy(true);
    try {
      await api.put("/api/v1/me/avatar-frame", { frame_id: id });
      setMsg(id === null ? t.removed : t.equipped);
      onChange?.(id);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
      setTimeout(() => setMsg(null), 2500);
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <div className={FRAME_GRID}>
        <div className={FRAME_CARD}>
          <span
            className={`avatar-frame-demo ${current === null ? "frame-on" : ""}`}
            aria-hidden
          >
            🙂
          </span>
          <p className="text-xs font-bold text-ink">{t.none}</p>
          <button
            className={FRAME_BTN_OFF}
            disabled={busy || current === null}
            onClick={() => equip(null)}
          >
            {t.btnRemove}
          </button>
        </div>
        {frames.map((f) => (
          <div key={f.id} className={FRAME_CARD}>
            <span
              className={
                "avatar-frame-demo frame-style-" +
                `${f.id} relative ${current === f.id ? "frame-on" : ""}`
              }
              style={f.image_url ? undefined : frameStyle(f.css)}
              aria-hidden
            >
              🙂
              {f.image_url ? (
                // eslint-disable-next-line @next/next/no-img-element
                <img src={f.image_url} alt="" className={FRAME_IMG} />
              ) : null}
            </span>
            <p className="text-xs font-bold text-ink">{f.name}</p>
            <p className="text-xs text-sub">
              {f.price > 0 ? `✨ ${f.price}` : t.free}
            </p>
            <button
              className={FRAME_BTN_ON}
              disabled={busy || current === f.id}
              onClick={() => equip(f.id)}
            >
              {current === f.id ? t.on : t.btnEquip}
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}

function frameStyle(css: string): React.CSSProperties {
  // css 形如 "border-color:#e14d4d" —— 仅放行 border-color/box-shadow 两个属性
  const style: Record<string, string> = {};
  for (const decl of css.split(";")) {
    const [k, v] = decl.split(":").map((s) => s?.trim());
    if (k && v && ["border-color", "box-shadow"].includes(k)) style[k] = v;
  }
  return style as React.CSSProperties;
}

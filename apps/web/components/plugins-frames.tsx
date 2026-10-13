"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { toast } from "@/components/ui/toast";

/** 同类站 插件移植前台（从 components/plugins.tsx 按域拆出）：
 *  头像挂件 FrameShop——挂件卡片网格 + 佩戴/取下。 */

export interface Frame {
  id: number;
  name: string;
  css: string;
  image_url?: string | null;
  price: number;
}

// 挂件卡片网格（自适应列；M6.2：<768 降单列——紧凑卡信息密度可读）
const FRAME_GRID =
  "grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] " +
  "gap-3 max-md:grid-cols-1";
// 卡片容器 / 卡片上的两种操作按钮
const FRAME_CARD = "brand-panel flex flex-col items-center gap-2 p-4";
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
      toast.success(id === null ? t.removed : t.equipped);
      onChange?.(id);
    } catch (e) {
      // A1 反馈协议：写操作结果走 toast（requestId 芯片必附）
      toast.error(
        e instanceof ApiError ? e.message : dict.common.networkError,
        {
        requestId: e instanceof ApiError ? e.requestId : undefined,
      });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-3">
      <div className={FRAME_GRID}>
        <div className={FRAME_CARD}>
          <span
            className={
              "avatar-frame-demo " + `${current === null ? "frame-on" : ""}`
            }
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
  // css 形如 "border-color:<hex>" —— 仅放行 border-color/box-shadow 两个属性
  // design-cov-exempt: 上一行 hex 是「站长存库 CSS 字符串」的格式示例，非 UI 配色
  const style: Record<string, string> = {};
  for (const decl of css.split(";")) {
    const [k, v] = decl.split(":").map((s) => s?.trim());
    if (k && v && ["border-color", "box-shadow"].includes(k)) style[k] = v;
  }
  return style as React.CSSProperties;
}

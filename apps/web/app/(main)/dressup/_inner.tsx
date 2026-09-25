"use client";

import { PANEL_LG } from "@/lib/ui-classes";

import { useCallback, useEffect, useState, useRef } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt, fmtCur } from "@/i18n/config";
import { FramePreview } from "@/components/frame-preview";

interface Dressup {
  item_id: number;
  name: string;
  kind: string;
  price: number;
  slot: string | null;
  owned: boolean;
  wearing: boolean;
  /** 0207：头像框 SKU 带 config（frame_id 用于列表预览） */
  config?: { frame_id?: number; effect?: string; [k: string]: unknown };
}

/** 装扮中心（M25）：购买走商店管线，佩戴同类互斥 */
export default function DressupPage() {
  const { dict, locale, currency } = useI18n();
  const [items, setItems] = useState<Dressup[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  // 幂等键按（用户+商品）在会话内固定：双击/网络重试共用同一键，杜绝双扣
  const idemRef = useRef<Record<number, string>>({});
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setItems(await api.get<Dressup[]>("/api/v1/dressup/list"));
    } catch (e) {
      setMsg(
        e instanceof ApiError && e.code === 2001
          ? dict.errors[2001]
          : dict.common.loadFailed,
      );
    }
  }, [dict]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  async function wear(item: Dressup, wear: boolean) {
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/dressup/wear", {
        item_id: item.item_id,
        wear,
      });
      setMsg(
        wear
          ? fmt(dict.dressup.worn, { name: item.name })
          : fmt(dict.dressup.unworn, { name: item.name }),
      );
      refresh();
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  async function buy(item: Dressup) {
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/shop/buy", {
        item_id: item.item_id,
        idempotency_key: (idemRef.current[item.item_id] ??=
          `dressup-${item.item_id}-${crypto.randomUUID()}`),
      });
      setMsg(
        fmtCur(
          dict.dressup.buyOk,
          { name: item.name, price: item.price },
          currency,
        ),
      );
      refresh();
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  const slots = ["avatar", "username"];

  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">Dress Up</div>
          <h1 className="font-display text-2xl">{dict.dressup.title}</h1>
        </div>
        <span className="sub">{dict.dressup.subtitle}</span>
      </div>

      {slots.map((slot) => {
        const slotItems = items.filter((i) => (i.slot ?? "avatar") === slot);
        if (slotItems.length === 0) return null;
        return (
          <section
            key={slot}
            className={PANEL_LG}
          >
            <h2 className="mb-3 font-display text-lg">
              {dict.dressup.slots[slot] ?? slot}
            </h2>
            <ul className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              {slotItems.map((d) => (
                <li
                  key={d.item_id}
                  className={`flex items-center gap-3 rounded-[var(--r-md)] border p-3 ${
                    d.wearing
                      ? "border-sun bg-sun/10"
                      : "border-line bg-[var(--surface-card)]"
                  }`}
                >
                  <span aria-hidden className="text-3xl">
                    {d.kind === "avatar_frame"
                      ? "🖼️"
                      : d.kind === "animated_avatar"
                        ? "✨"
                        : d.kind === "rainbow_id"
                          ? "🌈"
                          : "🎨"}
                  </span>
                  {d.kind === "avatar_frame" && d.config?.frame_id != null && (
                    <FramePreview frameId={d.config.frame_id} size={40} />
                  )}
                  <div className="flex-1">
                    <p className="text-sm font-bold">
                      {d.name}
                      {d.wearing && (
                        <span className="ml-1 rounded-full bg-sun px-2 py-0.5 text-[10px] text-ink">
                          {dict.dressup.wearing}
                        </span>
                      )}
                    </p>
                    <p className="text-xs text-sub">
                      {dict.dressup.kinds[d.kind] ?? d.kind} ·{" "}
                      {d.owned
                        ? dict.dressup.owned
                        : fmtCur(
                            dict.dressup.price,
                            {
                              n: d.price.toLocaleString(dateLocale(locale)),
                            },
                            currency,
                          )}
                    </p>
                  </div>
                  {d.owned ? (
                    <button
                      onClick={() => wear(d, !d.wearing)}
                      disabled={busy}
                      className="btn btn-sm"
                    >
                      {d.wearing ? dict.dressup.unwear : dict.dressup.wear}
                    </button>
                  ) : (
                    <button
                      onClick={() => buy(d)}
                      disabled={busy}
                      className="btn btn-sm btn-primary"
                    >
                      {dict.dressup.buy}
                    </button>
                  )}
                </li>
              ))}
            </ul>
          </section>
        );
      })}

      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
    </div>
  );
}

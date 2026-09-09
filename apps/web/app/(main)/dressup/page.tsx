"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";

interface Dressup {
  item_id: number;
  name: string;
  kind: string;
  price: number;
  slot: string | null;
  owned: boolean;
  wearing: boolean;
}

/** 装扮中心（M25）：购买走商店管线，佩戴同类互斥 */
export default function DressupPage() {
  const { dict, locale } = useI18n();
  const [items, setItems] = useState<Dressup[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
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
        idempotency_key: `dressup-${item.item_id}-${Date.now()}`,
      });
      setMsg(fmt(dict.dressup.buyOk, { name: item.name, price: item.price }));
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
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.dressup.title}</h1>
        <span className="text-sm text-sub">{dict.dressup.subtitle}</span>
      </div>

      {slots.map((slot) => {
        const slotItems = items.filter((i) => (i.slot ?? "avatar") === slot);
        if (slotItems.length === 0) return null;
        return (
          <section
            key={slot}
            className="rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]"
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
                      : "border-line bg-white"
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
                        : fmt(dict.dressup.price, {
                            n: d.price.toLocaleString(dateLocale(locale)),
                          })}
                    </p>
                  </div>
                  {d.owned ? (
                    <button
                      onClick={() => wear(d, !d.wearing)}
                      disabled={busy}
                      className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-ink active:scale-[0.97] disabled:opacity-50"
                    >
                      {d.wearing ? dict.dressup.unwear : dict.dressup.wear}
                    </button>
                  ) : (
                    <button
                      onClick={() => buy(d)}
                      disabled={busy}
                      className="min-h-[36px] rounded-full bg-coral px-4 text-xs font-bold text-white active:scale-[0.97] disabled:opacity-50"
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

"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

interface Dressup {
  item_id: number;
  name: string;
  kind: string;
  price: number;
  slot: string | null;
  owned: boolean;
  wearing: boolean;
}

const KIND_LABEL: Record<string, string> = {
  avatar_frame: "头像框",
  animated_avatar: "动态头像",
  rainbow_id: "彩虹 ID",
  rainbow_name: "彩虹用户名样式",
};

const SLOT_LABEL: Record<string, string> = {
  avatar: "头像位",
  username: "用户名位",
};

/** 装扮中心（M25）：购买走商店管线，佩戴同类互斥 */
export default function DressupPage() {
  const [items, setItems] = useState<Dressup[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setItems(await api.get<Dressup[]>("/api/v1/dressup/list"));
    } catch (e) {
      setMsg(e instanceof ApiError && e.code === 2001 ? "请先登录" : "加载失败");
    }
  }, []);

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
      setMsg(wear ? `已佩戴「${item.name}」` : `已摘下「${item.name}」`);
      refresh();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : "网络异常");
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
      setMsg(`购买成功「${item.name}」（- ${item.price} 火花）`);
      refresh();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : "网络异常");
    } finally {
      setBusy(false);
    }
  }

  const slots = ["avatar", "username"];

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">装扮中心</h1>
        <span className="text-sm text-sub">同类装扮同时只能佩戴一件</span>
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
              {SLOT_LABEL[slot] ?? slot}
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
                          佩戴中
                        </span>
                      )}
                    </p>
                    <p className="text-xs text-sub">
                      {KIND_LABEL[d.kind] ?? d.kind} ·{" "}
                      {d.owned ? "已拥有" : `${d.price.toLocaleString("zh-CN")} 火花`}
                    </p>
                  </div>
                  {d.owned ? (
                    <button
                      onClick={() => wear(d, !d.wearing)}
                      disabled={busy}
                      className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-ink active:scale-[0.97] disabled:opacity-50"
                    >
                      {d.wearing ? "摘下" : "佩戴"}
                    </button>
                  ) : (
                    <button
                      onClick={() => buy(d)}
                      disabled={busy}
                      className="min-h-[36px] rounded-full bg-coral px-4 text-xs font-bold text-white active:scale-[0.97] disabled:opacity-50"
                    >
                      购买
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

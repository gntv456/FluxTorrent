"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

/** 用户自购置顶/限时免费（0101，好学站插件口径）：
 *  档位 = sticky1 一级置顶 / sticky2 二级置顶 / free 限时免费 × 24/72 小时，
 *  价目来自 /promo/plans；购买走 /promo/buy（幂等键防双扣）。 */
interface PlanEntry {
  hours: number;
  price: number;
}

interface PlansResp {
  enabled: boolean;
  plans: Record<string, PlanEntry[]>;
}

const KIND_LABELS: Record<string, string> = {
  sticky1: "一级置顶",
  sticky2: "二级置顶",
  free: "限时免费",
};

export function PromoBuyButton({
  torrentId,
  isOwner,
}: {
  torrentId: number;
  isOwner: boolean;
}) {
  const { dict } = useI18n();
  const router = useRouter();
  const t = dict.promoBuy ?? {
    title: "推广本种子",
    pick: "选择档位",
    buy: "购买",
    busy: "处理中…",
    ok: "已生效",
    needOwner: "仅种子发布者可购买",
    disabled: "功能未开放",
  };
  const [plans, setPlans] = useState<PlansResp | null>(null);
  const [pick, setPick] = useState<string>("");
  const [msg, setMsg] = useState<string | null>(null);
  const [okMsg, setOkMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const r = await api.get<PlansResp>("/api/v1/promo/plans");
      setPlans(r);
      const first = Object.entries(r.plans).find(([, v]) => v.length > 0);
      if (first) setPick(`${first[0]}:${first[1][0].hours}`);
    } catch {
      setPlans({ enabled: false, plans: {} });
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  if (plans && !plans.enabled) return null;
  if (!plans) return null;
  if (!isOwner) return <p className="text-xs text-sub">{t.needOwner}</p>;

  async function buy() {
    if (!pick) return;
    setBusy(true);
    setMsg(null);
    setOkMsg(null);
    const [kind, hours] = pick.split(":");
    try {
      await api.post("/api/v1/promo/buy", {
        torrent_id: torrentId,
        kind,
        hours: Number(hours),
        idempotency_key: `web-${torrentId}-${kind}-${crypto.randomUUID()}`,
      });
      setOkMsg(t.ok);
      router.refresh();
    } catch (err) {
      setMsg(
        err instanceof ApiError
          ? err.message
          : (dict.common.networkError ?? "失败"),
      );
    } finally {
      setBusy(false);
    }
  }

  const entries = Object.entries(plans.plans).flatMap(([kind, list]) =>
    list.map((e) => ({ key: `${kind}:${e.hours}`, kind, ...e })),
  );
  if (entries.length === 0) return null;

  return (
    <div className="flex flex-col gap-1 rounded-[var(--r-sm)] border border-line bg-cloud p-2">
      <span className="text-xs font-bold">{t.title}</span>
      <select
        aria-label={t.pick}
        className="min-h-[36px] rounded-[var(--r-sm)] border border-line bg-white px-2 text-xs"
        value={pick}
        onChange={(e) => setPick(e.target.value)}
      >
        {entries.map((e) => (
          <option key={e.key} value={e.key}>
            {KIND_LABELS[e.kind] ?? e.kind} {e.hours}h — {e.price} 魔力
          </option>
        ))}
      </select>
      <button
        type="button"
        disabled={busy || !pick}
        className="min-h-[36px] rounded-full bg-sun px-3 text-xs font-bold text-ink active:scale-[0.97] disabled:opacity-50"
        onClick={() => void buy()}
      >
        {busy ? t.busy : t.buy}
      </button>
      {msg && <p className="text-xs text-danger">{msg}</p>}
      {okMsg && <p className="text-xs text-mint">{okMsg}</p>}
    </div>
  );
}

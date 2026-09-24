"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { Modal } from "@/components/modal";

/** 用户自购置顶/限时免费（0101，好学站插件口径）：
 *  档位 = sticky1 一级置顶 / sticky2 二级置顶 / free 限时免费 × 24/72 小时，
 *  价目来自 /promo/plans；购买走 /promo/buy（幂等键防双扣）。
 *  0173：详情页头部只留「购买置顶免费」按钮，档位选择进覆盖式弹窗。 */
interface PlanEntry {
  hours: number;
  price: number;
}

interface PlansResp {
  enabled: boolean;
  plans: Record<string, PlanEntry[]>;
}

const KIND_ORDER = ["sticky1", "sticky2", "free"] as const;

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
    title: "购买置顶免费",
    pick: "选择档位",
    buy: "购买",
    busy: "处理中…",
    ok: "已生效",
    needOwner: "仅种子发布者可购买",
    disabled: "功能未开放",
  };
  const [plans, setPlans] = useState<PlansResp | null>(null);
  const [open, setOpen] = useState(false);
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
        err instanceof ApiError ? err.message : dict.common.networkError,
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
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        className="min-h-[36px] rounded-full bg-sun px-4 text-xs font-bold text-ink active:scale-[0.97]"
      >
        {t.title}
      </button>
      <Modal open={open} onClose={() => setOpen(false)} title={t.title}>
        <div className="flex flex-col gap-3">
          {KIND_ORDER.map((kind) => {
            const list = plans.plans[kind] ?? [];
            if (list.length === 0) return null;
            return (
              <fieldset
                key={kind}
                className="flex flex-col gap-1.5 text-xs"
              >
                <legend className="font-bold">
                  {KIND_LABELS[kind] ?? kind}
                </legend>
                <div className="flex flex-wrap gap-2">
                  {list.map((e) => {
                    const key = `${kind}:${e.hours}`;
                    const on = pick === key;
                    return (
                      <button
                        key={key}
                        type="button"
                        aria-pressed={on}
                        onClick={() => setPick(key)}
                        className={
                          "min-h-[36px] rounded-full border px-3 " +
                          "text-xs font-bold transition-colors " +
                          (on
                            ? "border-sun bg-sun text-ink"
                            : "border-line text-sub hover:border-sky")
                        }
                      >
                        {e.hours}h · {e.price}
                        {" "}
                        {fmt(dict.common.spark, {
                          magic: e.price,
                        })}
                      </button>
                    );
                  })}
                </div>
              </fieldset>
            );
          })}
          <div className="flex items-center gap-2 pt-1">
            <button
              type="button"
              disabled={busy || !pick}
              onClick={() => void buy()}
              className="min-h-[36px] rounded-full bg-sky-deep px-5 text-xs font-bold text-white disabled:opacity-50"
            >
              {busy ? t.busy : t.buy}
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => setOpen(false)}
              className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-sub disabled:opacity-50"
            >
              {dict.common.cancel}
            </button>
          </div>
          {msg && <p className="text-xs text-danger">{msg}</p>}
          {okMsg && <p className="text-xs text-mint">{okMsg}</p>}
        </div>
      </Modal>
    </>
  );
}

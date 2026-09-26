"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { Modal } from "@/components/modal";

/** 用户自购置顶/限时免费（0101，好学站插件口径；0213 起档位读服务端注册表）：
 *  档位由 GET /promo/plans 返回的 kinds 元数据驱动（站方可自定义增删），
 *  价目同响应；购买走 /promo/buy（幂等键防双扣）。
 *  0173：详情页头部只留「购买置顶免费」按钮，档位选择进覆盖式弹窗。 */
interface PlanEntry {
  hours: number;
  price: number;
}

interface KindMeta {
  kind: string;
  label_zh: string;
  label_en: string;
  effect: string;
  i18n_key: string | null;
}

interface PlansResp {
  enabled: boolean;
  plans: Record<string, PlanEntry[]>;
  kinds?: KindMeta[];
}

/** 档位显示名：优先字典（i18n_key → promoKind.*），回落服务端 label（按站点语言口径） */
function kindLabel(k: KindMeta, dict: ReturnType<typeof useI18n>["dict"]) {
  const map = dict.promoKind as unknown as
    | Record<string, string>
    | undefined;
  const key = k.i18n_key ?? k.kind;
  return map?.[key] ?? k.label_zh ?? k.kind;
}

export function PromoBuyButton({ torrentId }: { torrentId: number }) {
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
      setPlans({ enabled: false, plans: {}, kinds: [] });
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  if (plans && !plans.enabled) return null;
  if (!plans) return null;

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
  // 档位顺序：服务端 kinds 元数据为准（站方在后台拖/设 sort_order）；
  // 元数据缺失（老响应/降级）时回落 plans 键序
  const orderedKinds: KindMeta[] =
    plans.kinds && plans.kinds.length > 0
      ? plans.kinds.filter((k) => (plans.plans[k.kind] ?? []).length > 0)
      : Object.keys(plans.plans).map((k) => ({
          kind: k,
          label_zh: k,
          label_en: k,
          effect: "",
          i18n_key: k,
        }));
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
          {orderedKinds.map((k) => {
            const list = plans.plans[k.kind] ?? [];
            if (list.length === 0) return null;
            return (
              <fieldset
                key={k.kind}
                className="flex flex-col gap-1.5 text-xs"
              >
                <legend className="font-bold">
                  {kindLabel(k, dict)}
                </legend>
                <div className="flex flex-wrap gap-2">
                  {list.map((e) => {
                    const key = `${k.kind}:${e.hours}`;
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

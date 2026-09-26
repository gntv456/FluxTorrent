"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import {
  PK_FCOL_CLS,
  PK_ROW_BTN_DANGER_CLS,
  PK_SUB_TITLE_CLS,
  PK_TD_CLS,
  PK_TH_CLS,
  PromoKindResp,
} from "./admin-promo-shared";
import { BTN_MD_SKY, INPUT_CLOUD } from "@/lib/ui-classes";

/** 价目档面板（0213）：kind × 时长 → 价格。
 *  纯展示 + 回调，数据由父组件 AdminPromoKinds 提供（共用一份请求/消息）。 */
interface Props {
  state: {
    data: PromoKindResp;
    busy: boolean;
    run: (fn: () => Promise<void>, after?: string) => Promise<void>;
  };
}

export function AdminPromoTiers({ state }: Props) {
  const { dict } = useI18n();
  const t = dict.adminPromoKinds;
  const { data, busy, run } = state;
  const [tKind, setTKind] = useState("");
  const [tHours, setTHours] = useState(24);
  const [tPrice, setTPrice] = useState(1000);

  // 档位列表到位后给价目表单选个默认档
  useEffect(() => {
    if (!tKind && data.kinds.length) setTKind(data.kinds[0].kind);
  }, [data, tKind]);

  const addTier = () =>
    run(
      () =>
        api.post("/api/v1/admin/promo-kinds/tiers", {
          kind: tKind,
          hours: tHours,
          price: tPrice,
        }),
      t.tierSaved,
    );

  const delTier = (id: number) =>
    run(() => api.del(`/api/v1/admin/promo-kinds/tiers/${id}`), t.deleted);

  return (
    <>
      <h3 className={PK_SUB_TITLE_CLS}>{t.tiersTitle}</h3>
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className={`${PK_TH_CLS} w-28`}>{t.colKind}</td>
              <td className={`${PK_TH_CLS} w-24`}>{t.colHours}</td>
              <td className={`${PK_TH_CLS} w-28`}>{t.colPrice}</td>
              <td className={`${PK_TH_CLS} w-16`}>{t.colEnabled}</td>
              <td className={`${PK_TH_CLS} w-20`} />
            </tr>
          </thead>
          <tbody>
            {data.tiers.map((r) => (
              <tr key={r.id}>
                <td className={PK_TD_CLS}>{r.kind}</td>
                <td className={PK_TD_CLS}>{r.hours}h</td>
                <td className={PK_TD_CLS}>{r.price}</td>
                <td className={PK_TD_CLS}>{r.enabled ? t.yes : t.no}</td>
                <td className={PK_TD_CLS}>
                  <button
                    disabled={busy}
                    onClick={() => void delTier(r.id)}
                    className={PK_ROW_BTN_DANGER_CLS}
                  >
                    {t.del}
                  </button>
                </td>
              </tr>
            ))}
            {data.tiers.length === 0 && (
              <tr>
                <td colSpan={5} className="py-4 text-center text-sub">
                  {t.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="mt-3 flex flex-wrap items-end gap-2">
        <label className={PK_FCOL_CLS}>
          <span className="text-xs text-sub">{t.fldKind}</span>
          <select
            value={tKind}
            onChange={(e) => setTKind(e.target.value)}
            className={INPUT_CLOUD}
          >
            {data.kinds.map((k) => (
              <option key={k.kind} value={k.kind}>
                {t.labels[k.kind] ?? k.label_zh}
              </option>
            ))}
          </select>
        </label>
        <label className={PK_FCOL_CLS}>
          <span className="text-xs text-sub">{t.fldHours}</span>
          <input
            type="number"
            min={1}
            value={tHours}
            onChange={(e) => setTHours(Number(e.target.value))}
            className={`${INPUT_CLOUD} w-24`}
          />
        </label>
        <label className={PK_FCOL_CLS}>
          <span className="text-xs text-sub">{t.fldPrice}</span>
          <input
            type="number"
            min={1}
            value={tPrice}
            onChange={(e) => setTPrice(Number(e.target.value))}
            className={`${INPUT_CLOUD} w-28`}
          />
        </label>
        <button
          disabled={busy || !tKind}
          onClick={() => void addTier()}
          className={BTN_MD_SKY}
        >
          {t.addTierBtn}
        </button>
      </div>
    </>
  );
}

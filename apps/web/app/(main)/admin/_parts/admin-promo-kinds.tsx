"use client";

import { useCallback, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { usePromoKindData } from "./use-promo-kind-data";
import { AdminPromoTiers } from "./admin-promo-tiers";
import {
  KindRow,
  PK_FCOL_CLS,
  PK_INTRO_CLS,
  PK_META_CLS,
  PK_ROW_BTN_DANGER_CLS,
  PK_ROW_BTN_MR_CLS,
  PK_SUB_TITLE_CLS,
  PK_TD_CLS,
  PK_TH_CLS,
  PK_TITLE_CLS,
} from "./admin-promo-shared";
import { BTN_MD_SKY, INPUT_CLOUD } from "@/lib/ui-classes";

/** 促销档位注册表管理（0213）：用户自购推广的「档位 × 时长 → 价格」可视化配置。
 *  服务「不偏向任何 PT 类型」——站方可自由增删档位（如给影视站加「提升下载量」、
 *  给综合站加「求种高亮」），落地效果由 effect 驱动，与 worker 计费倍率同口径。
 *  挂载点：/admin?tool=freeleech 页面下方（与站方级促销同域）。
 *  价目档面板在 ./admin-promo-tiers.tsx（300 行门禁拆出，共用本组件数据）。 */

export function AdminPromoKinds() {
  const { dict } = useI18n();
  const t = dict.adminPromoKinds;
  const { data, msg, setMsg, busy, run } = usePromoKindData();
  const [nKind, setNKind] = useState("");
  const [nLabel, setNLabel] = useState("");
  const [nEffect, setNEffect] = useState("free");

  const addKind = useCallback(
    () =>
      run(async () => {
        await api.post("/api/v1/admin/promo-kinds", {
          kind: nKind.trim(),
          label_zh: nLabel.trim(),
          effect: nEffect,
          enabled: true,
        });
        setNKind("");
        setNLabel("");
      }, t.added),
    [run, nKind, nLabel, nEffect, t.added],
  );

  const toggleKind = useCallback(
    (k: KindRow) =>
      run(
        () =>
          api.put(`/api/v1/admin/promo-kinds/${k.kind}`, {
            kind: k.kind,
            label_zh: k.label_zh,
            effect: k.effect,
            enabled: !k.enabled,
          }),
        t.saved,
      ),
    [run, t.saved],
  );

  const delKind = useCallback(
    async (k: KindRow) => {
      // 删除可能因「已有购买记录」降级为停用，需看响应决定提示文案
      let downgraded = false;
      await run(async () => {
        const r = await api.del<{ disabled?: boolean }>(
          `/api/v1/admin/promo-kinds/${k.kind}`,
        );
        downgraded = !!r?.disabled;
      });
      if (downgraded) setMsg(t.disabledHint);
    },
    [run, setMsg, t.disabledHint],
  );

  if (!data) return null;

  return (
    <section className="baozi-panel mt-4 p-4">
      <h2 className={PK_TITLE_CLS}>{t.title}</h2>
      <p className={PK_INTRO_CLS}>{t.intro}</p>
      {msg && <p className={PK_META_CLS}>{msg}</p>}

      <h3 className={PK_SUB_TITLE_CLS}>{t.kindsTitle}</h3>
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className={`${PK_TH_CLS} w-28`}>{t.colKind}</td>
              <td className={PK_TH_CLS}>{t.colLabel}</td>
              <td className={`${PK_TH_CLS} w-28`}>{t.colEffect}</td>
              <td className={`${PK_TH_CLS} w-16`}>{t.colEnabled}</td>
              <td className={`${PK_TH_CLS} w-32`} />
            </tr>
          </thead>
          <tbody>
            {data.kinds.map((k) => (
              <tr key={k.kind}>
                <td className={PK_TD_CLS}>{k.kind}</td>
                <td className={PK_TD_CLS}>
                  {t.labels[k.kind] ?? k.label_zh}
                </td>
                <td className={PK_TD_CLS}>
                  {t.effects[k.effect] ?? k.effect}
                </td>
                <td className={PK_TD_CLS}>{k.enabled ? t.yes : t.no}</td>
                <td className={`${PK_TD_CLS} whitespace-nowrap`}>
                  <button
                    disabled={busy}
                    onClick={() => void toggleKind(k)}
                    className={PK_ROW_BTN_MR_CLS}
                  >
                    {k.enabled ? t.disable : t.enable}
                  </button>
                  <button
                    disabled={busy}
                    onClick={() => void delKind(k)}
                    className={PK_ROW_BTN_DANGER_CLS}
                  >
                    {t.del}
                  </button>
                </td>
              </tr>
            ))}
            {data.kinds.length === 0 && (
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
          <input
            value={nKind}
            onChange={(e) => setNKind(e.target.value)}
            placeholder={t.kindPlaceholder}
            className={`${INPUT_CLOUD} w-32`}
          />
        </label>
        <label className={PK_FCOL_CLS}>
          <span className="text-xs text-sub">{t.fldLabel}</span>
          <input
            value={nLabel}
            onChange={(e) => setNLabel(e.target.value)}
            className={`${INPUT_CLOUD} w-36`}
          />
        </label>
        <label className={PK_FCOL_CLS}>
          <span className="text-xs text-sub">{t.fldEffect}</span>
          <select
            value={nEffect}
            onChange={(e) => setNEffect(e.target.value)}
            className={INPUT_CLOUD}
          >
            {data.effects.map((e) => (
              <option key={e} value={e}>
                {t.effects[e] ?? e}
              </option>
            ))}
          </select>
        </label>
        <button
          disabled={busy || !nKind.trim() || !nLabel.trim()}
          onClick={() => void addKind()}
          className={BTN_MD_SKY}
        >
          {t.addKindBtn}
        </button>
      </div>

      <AdminPromoTiers state={{ data, busy, run }} />
    </section>
  );
}

"use client";

import { api } from "@/lib/api-client";
import { BTN_SM_BOLD } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";
import type { SitePromo } from "./staff-tools-ops-types";

/** 运营工具·置顶/优惠 promo tab（从 staff-tools-ops.tsx 按域拆出）。 */

function toLocalInput(iso: string): string {
  const d = new Date(iso);
  return new Date(d.getTime() - d.getTimezoneOffset() * 60000)
    .toISOString()
    .slice(0, 16);
}

function scopeLabel(
  t: ReturnType<typeof useI18n>["dict"]["stafftools"],
  p: SitePromo,
) {
  if (p.scope === "global") return t.scopeGlobal;
  if (p.scope === "official") return t.scopeOfficial;
  if (p.scope === "non_official") return t.scopeNonOfficial;
  if (p.scope === "category")
    return `${t.scopeCategory} · ${p.category_name ?? `#${p.category_id}`}`;
  return p.scope;
}

const EDIT_BTN_CLS =
  "min-h-[28px] rounded-full border border-line px-3 text-xs " +
  "font-bold text-sky";

const DEL_BTN_CLS =
  "min-h-[28px] rounded-full border border-line px-3 text-xs " +
  "font-bold text-danger";

export function OpsPromoTab(props: {
  promo: SitePromo[];
  promoEditId: number | null;
  setPromoEditId: (v: number | null) => void;
  promoScope: string;
  setPromoScope: (v: string) => void;
  promoCat: number | "";
  setPromoCat: (v: number | "") => void;
  cats: { id: number; name: string }[];
  promoStart: string;
  setPromoStart: (v: string) => void;
  promoEnd: string;
  setPromoEnd: (v: string) => void;
  promoKind: string;
  setPromoKind: (v: string) => void;
  promoHours: number;
  setPromoHours: (v: number) => void;
  busy: boolean;
  guard: (fn: () => Promise<void>, ok: string) => Promise<void>;
}) {
  const {
    promo,
    promoEditId,
    setPromoEditId,
    promoScope,
    setPromoScope,
    promoCat,
    setPromoCat,
    cats,
    promoStart,
    setPromoStart,
    promoEnd,
    setPromoEnd,
    promoKind,
    setPromoKind,
    promoHours,
    setPromoHours,
    busy,
    guard,
  } = props;
  const { dict } = useI18n();
  const t = dict.stafftools;
  return (
    <>
      <section className="baozi-panel p-4">
        <h2 className="mb-3 text-base font-bold text-ink">{t.promoNew}</h2>
        <div className="cmgmt-form">
          <label>
            {t.promoScope}
            <select
              value={promoScope}
              onChange={(e) => {
                setPromoScope(e.target.value);
                setPromoCat("");
              }}
            >
              <option value="global">{t.scopeGlobal}</option>
              <option value="official">{t.scopeOfficial}</option>
              <option value="non_official">{t.scopeNonOfficial}</option>
              <option value="category">{t.scopeCategory}</option>
            </select>
          </label>
          {promoScope === "category" && (
            <label>
              {t.promoCat}
              <select
                value={promoCat}
                onChange={(e) =>
                  setPromoCat(
                    e.target.value === "" ? "" : Number(e.target.value),
                  )
                }
              >
                <option value="">{t.promoCat}</option>
                {cats.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label>
            {t.promoStart}
            <input
              type="datetime-local"
              value={promoStart}
              onChange={(e) => setPromoStart(e.target.value)}
            />
          </label>
          <label>
            {t.promoEnd}
            <input
              type="datetime-local"
              value={promoEnd}
              onChange={(e) => setPromoEnd(e.target.value)}
            />
          </label>
          <label>
            {t.promoKind}
            <select
              value={promoKind}
              onChange={(e) => setPromoKind(e.target.value)}
            >
              <option value="free">Free 免费</option>
              <option value="x2">2x 上传</option>
              <option value="x2free">2x 免费</option>
              <option value="half">50% 下载</option>
              <option value="x2half">2x 50%</option>
              <option value="p30">30% 下载</option>
            </select>
          </label>
          <label>
            {t.promoHours}（未填结束时间时生效）
            <input
              type="number"
              min={1}
              max={720}
              value={promoHours}
              onChange={(e) => setPromoHours(Number(e.target.value))}
            />
          </label>
          <div className="flex gap-2">
            <button
              className="baozi-button"
              disabled={
                busy ||
                promoHours < 1 ||
                (promoScope === "category" && promoCat === "")
              }
              onClick={() =>
                guard(
                  async () => {
                    const payload = {
                      kind: promoKind,
                      hours: promoHours,
                      scope: promoScope,
                      ...(promoScope === "category"
                        ? { category_id: promoCat }
                        : {}),
                      ...(promoStart
                        ? { starts_at: new Date(promoStart).toISOString() }
                        : {}),
                      ...(promoEnd
                        ? { ends_at: new Date(promoEnd).toISOString() }
                        : {}),
                    };
                    if (promoEditId === null) {
                      await api.post("/api/v1/admin/freeleech", payload);
                    } else {
                      await api.put(
                        `/api/v1/admin/freeleech/${promoEditId}`,
                        payload,
                      );
                      setPromoEditId(null);
                    }
                    setPromoCat("");
                  },
                  promoEditId === null ? t.promoSet : "已保存",
                )
              }
            >
              {promoEditId === null ? t.promoBtnSet : "保存修改"}
            </button>
            <button
              className={BTN_SM_BOLD}
              disabled={busy || promo.length === 0}
              onClick={() =>
                guard(async () => {
                  await api.del("/api/v1/admin/freeleech");
                }, t.promoCleared)
              }
            >
              {t.promoBtnClear}
            </button>
          </div>
        </div>
      </section>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">{t.colScope}</td>
            <td className="colhead">{t.promoKind}</td>
            <td className="colhead">{t.promoStart}</td>
            <td className="colhead">{t.promoEnd}</td>
            <td className="colhead w-32" />
          </tr>
        </thead>
        <tbody>
          {promo.length > 0 ? (
            promo.map((p) => (
              <tr
                key={p.id}
                className={promoEditId === p.id ? "bg-sky-soft" : ""}
              >
                <td className="font-bold">{scopeLabel(t, p)}</td>
                <td className="font-bold">{p.kind}</td>
                <td className="text-xs text-sub">
                  {new Date(p.starts_at).toLocaleString("zh-CN")}
                </td>
                <td className="text-xs text-sub">
                  {new Date(p.ends_at).toLocaleString("zh-CN")}
                </td>
                <td>
                  <button
                    className={EDIT_BTN_CLS}
                    onClick={() => {
                      if (promoEditId === p.id) {
                        setPromoEditId(null);
                        return;
                      }
                      setPromoEditId(p.id);
                      setPromoScope(p.scope);
                      setPromoCat(
                        p.scope === "category" ? (p.category_id ?? "") : "",
                      );
                      setPromoKind(p.kind);
                      setPromoStart(toLocalInput(p.starts_at));
                      setPromoEnd(toLocalInput(p.ends_at));
                    }}
                  >
                    {promoEditId === p.id ? "取消" : "编辑"}
                  </button>
                  <button
                    className={`ml-1 ${DEL_BTN_CLS}`}
                    onClick={() =>
                      guard(async () => {
                        await api.del(`/api/v1/admin/freeleech/${p.id}`);
                        if (promoEditId === p.id) setPromoEditId(null);
                      }, "已删除")
                    }
                  >
                    删除
                  </button>
                </td>
              </tr>
            ))
          ) : (
            <tr>
              <td colSpan={5} className="py-6 text-center text-sub">
                {t.promoNone}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </>
  );
}

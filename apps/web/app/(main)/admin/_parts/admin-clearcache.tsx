/** 清除缓存面板（从 admin-freeleech.tsx 拆出，300 行门禁）：
 *  对应 POST /admin/clearcache（清除限流等运行期缓存键） */
import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage } from "@/i18n/client";
import { useI18n } from "@/i18n/client";
import { BTN_MD_SKY } from "@/lib/ui-classes";

const fmt = (tpl: string, x: Record<string, unknown>) =>
  tpl.replace(/\{(\w+)\}/g, (_, k) => String(x[k] ?? ""));

export function ClearCachePanel() {
  const { dict } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  const t = dict.adminPromo;
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function run() {
    setBusy(true);
    try {
      const r = await api.post<{ cleared: number }>(
        "/api/v1/admin/clearcache",
        {},
      );
      setMsg(fmt(t.cacheCleared, { n: r.cleared }));
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? (apiErrorMessage(dict, e))
          : a.actionFailed,
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="brand-panel p-4">
      <h2 className="mb-2 text-base font-bold">{t.cacheTitle}</h2>
      <p className="mb-3 text-xs text-sub">{t.cacheIntro}</p>
      {msg && (
        <p
          className={
            "mb-3 rounded-[var(--r-md)] bg-[var(--surface-sunken)] " +
            "p-2 text-xs"
          }
        >
          {msg}
        </p>
      )}
      <button
        disabled={busy}
        onClick={run}
        className={BTN_MD_SKY}
      >
        {t.cacheBtn}
      </button>
    </section>
  );
}

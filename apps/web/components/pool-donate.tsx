"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

/** 站免池捐赠表单（M13 众筹触发下月双免）：走 spark 支出管线，成功后刷新进度 */
export function PoolDonate() {
  const { dict, currency } = useI18n();
  const router = useRouter();
  const [amount, setAmount] = useState("1000");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const t = dict.magicPool;

  async function donate() {
    const n = parseInt(amount, 10);
    if (!Number.isFinite(n) || n <= 0) {
      setMsg(t.amountInvalid.replace("{magic}", currency));
      return;
    }
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/magic-pool/donate", {
        amount: n,
      });
      setMsg(t.donatedOk);
      router.refresh();
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mt-4 flex flex-wrap items-center gap-2">
      <input
        value={amount}
        onChange={(e) => setAmount(e.target.value.replace(/[^\d]/g, ""))}
        inputMode="numeric"
        aria-label={t.amountLabel.replace("{magic}", currency)}
        className="num w-32 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-3 py-2 text-sm"
      />
      {[1000, 5000, 20000].map((v) => (
        <button
          key={v}
          type="button"
          onClick={() => setAmount(String(v))}
          className="num min-h-[36px] rounded-full border border-line bg-[var(--surface-card)] px-3 text-xs text-sub active:scale-[0.97]"
        >
          {v.toLocaleString("zh-CN")}
        </button>
      ))}
      <button
        type="button"
        disabled={busy}
        onClick={donate}
        className="min-h-[44px] rounded-full bg-coral px-5 text-sm font-bold text-white active:scale-[0.97] disabled:opacity-50"
      >
        {busy ? t.donating : t.donate.replace("{magic}", currency)}
      </button>
      {msg && (
        <span className="text-xs text-sub" role="status">
          {msg}
        </span>
      )}
    </div>
  );
}

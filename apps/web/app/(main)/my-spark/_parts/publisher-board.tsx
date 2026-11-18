"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface LedgerResp {
  rows?: { amount: number; kind: string }[];
  total?: number;
}

/** 发布者看板（0498 前端收口，产品体检 A1）：免罪分成累计 + 三档加成说明。
 *  数据源 GET /me/spark/ledger?kind=hr_pardon_share（后端既有，此前无 UI）；
 *  三档系数是站点设定（seeding_publisher_mult_*），只展示口径不回传数值——
 *  系数站长可调，前端读 /me/spark 的预估已隐含加成，这里给「规则说明卡」。 */
export function PublisherBoard() {
  const { dict } = useI18n();
  const t = dict.pubboard;
  const [total, setTotal] = useState<number | null>(null);
  const [count, setCount] = useState(0);

  useEffect(() => {
    let alive = true;
    api
      .get<LedgerResp>("/api/v1/me/spark/ledger?kind=hr_pardon_share&per=100")
      .then((r) => {
        if (!alive || !r) return;
        const rows = r.rows ?? [];
        setTotal(rows.reduce((s, x) => s + (x.amount || 0), 0));
        setCount(rows.length);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  return (
    <div className="flex flex-col gap-4">
      <section className="brand-panel p-4">
        <h3 className="font-semibold mb-1">{t.shareTotal}</h3>
        <p className="text-xs opacity-60 mb-2">{t.shareNote}</p>
        {total === null ? (
          <p className="text-sm opacity-60">…</p>
        ) : count === 0 ? (
          <p className="text-sm opacity-60">{t.empty}</p>
        ) : (
          <p className="text-2xl font-display">
            {total.toLocaleString()}{" "}
            <span className="text-sm font-normal opacity-70">{t.unit}</span>
          </p>
        )}
      </section>
      <section className="brand-panel p-4">
        <h3 className="font-semibold mb-1">{t.decayTitle}</h3>
        <p className="text-xs opacity-60 mb-2">{t.decayNote}</p>
        <div className="grid gap-2 sm:grid-cols-2">
          {[
            [t.decayDay, "×4.0"],
            [t.decayMonth, "×2.5"],
            [t.decayYear, "×1.5"],
            [t.decayBase, "×1.0"],
          ].map(([label, mult]) => (
            <div
              key={label}
              className="flex justify-between text-sm rounded border
                border-[var(--line)] px-3 py-2"
            >
              <span>{label}</span>
              <span className="font-medium">{mult}</span>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}

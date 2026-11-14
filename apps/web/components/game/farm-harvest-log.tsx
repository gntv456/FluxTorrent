"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { PANEL_LG } from "@/lib/ui-classes";

/**
 * 收成记录（波1 P0）：农场此前全站唯一没有任何历史的玩法——
 * `/games/rounds` 以负数下注行为锚，收获是纯入账行进不了那个形状；
 * `farm_harvests` 表本来就在写，后端新读口 GET /farm/harvests 补上。
 * 自含取数 + 空态，field tab 一行挂载（farm/_inner 行数基线钉死）。
 */

interface HarvestRow {
  crop: string;
  earn: number;
  price: number;
  doubled: boolean;
  at: string;
}

export function FarmHarvestLog() {
  const { dict, currency } = useI18n();
  const tf = dict.farm as unknown as Record<string, string>;
  const [rows, setRows] = useState<HarvestRow[] | null>(null);

  const load = useCallback(async () => {
    try {
      setRows(await api.get<HarvestRow[]>("/api/v1/farm/harvests"));
    } catch {
      setRows([]);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  if (!rows?.length) return null;
  return (
    <div className={PANEL_LG}>
      <h2 className="mb-2 font-display text-base">{tf.harvestLogCap}</h2>
      <div className="flex flex-col gap-1">
        {rows.slice(0, 10).map((r, i) => (
          <div
            key={i}
            className="flex items-baseline justify-between gap-3 text-xs"
          >
            <span className="font-bold">
              {r.crop}
              {r.doubled && (
                <span className="ml-1 text-[var(--warning)]">×2</span>
              )}
            </span>
            <span className="num text-sub">
              {(r.at ?? "").slice(5, 16).replace("T", " ")}
            </span>
            <b className={`num ${r.earn > 0 ? "text-mint" : "text-sub"}`}>
              +{r.earn.toLocaleString()} {currency}
            </b>
          </div>
        ))}
      </div>
    </div>
  );
}

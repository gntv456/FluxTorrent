"use client";

/**
 * 宠物互动记录页（样图⑨「互动记录」子页）：今日统计 + 近期互动列表。
 * 数据 GET /games/pet/log（audit_log 聚合，只读）。
 */
import { useEffect, useState } from "react";
import Link from "next/link";
import { PANEL_LG } from "@/lib/ui-classes";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface PetLog {
  today: { feed: number; claim: number; customize: number };
  items: { action: string; at: string }[];
}

/** 面板形态：嵌入宠物主页「记录」tab（无页面头） */
export function PetLogPanel() {
  return <PetLogBody embedded />;
}

export function PetLogPage() {
  return <PetLogBody />;
}

function PetLogBody({ embedded = false }: { embedded?: boolean }) {
  const { dict } = useI18n();
  const t = dict.games.pet as Record<string, string>;
  const sub = dict.games.sub;
  const [data, setData] = useState<PetLog | null>(null);

  useEffect(() => {
    void api
      .get<PetLog>("/api/v1/games/pet/log")
      .then(setData)
      .catch(() => {});
  }, []);

  const actionLabel = (a: string) =>
    a === "feed" ? t.logFeed : a === "claim" ? t.logClaim : t.logCustomize;

  return (
    <div className="flex flex-col gap-4">
      {!embedded && (
        <div className="flex flex-wrap items-center gap-2">
          <Link
            href="/games/pet"
            className="min-h-[36px] rounded-full border border-line px-3 text-xs font-bold text-sub"
          >
            ← {t.title}
          </Link>
          <h1 className="font-display text-2xl">{t.logTitle}</h1>
        </div>
      )}
      {data && (
        <div className="sw-stat-row">
          <div className="sw-stat">
            <div className="sw-stat-lb">{t.logFeed}</div>
            <div className="sw-stat-vl num">{data.today.feed}</div>
          </div>
          <div className="sw-stat">
            <div className="sw-stat-lb">{t.logClaim}</div>
            <div className="sw-stat-vl num gold">{data.today.claim}</div>
          </div>
          <div className="sw-stat">
            <div className="sw-stat-lb">{t.logCustomize}</div>
            <div className="sw-stat-vl num">{data.today.customize}</div>
          </div>
        </div>
      )}
      <section className={PANEL_LG}>
        <h2 className="mb-3 font-display text-base">{sub.recentTitle}</h2>
        {!data ? (
          <p className="text-sm text-sub">…</p>
        ) : data.items.length === 0 ? (
          <p className="text-sm text-sub">{t.logEmpty}</p>
        ) : (
          <div className="flex flex-col gap-1.5">
            {data.items.map((r, i) => (
              <div
                key={i}
                className="flex items-center gap-3 rounded-[10px] bg-[var(--surface-raised)] px-3 py-2 text-xs"
              >
                <span className="num shrink-0 text-sub">
                  {new Date(r.at).toLocaleString()}
                </span>
                <span className="ml-auto font-bold">
                  {actionLabel(r.action)}
                </span>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}

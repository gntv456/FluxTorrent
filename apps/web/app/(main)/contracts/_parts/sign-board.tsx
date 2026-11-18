"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { BTN_SM_BOLD } from "@/lib/ui-classes";
import type { Dict } from "@/i18n/zh-CN";

interface Tier {
  kind: string;
  label: string;
  target: number;
  reward: number;
  penalty: number;
  sort: number;
}

export interface TiersResp {
  enabled: boolean;
  eligible: boolean;
  progress: null | {
    kind: string;
    target: number;
    current: number;
    recorded: number;
  };
  tiers: Tier[];
}

/** 契约签约台：价目卡 + 当月进度 + 签约/续约动作。
 *  计量口径随卡公示（产品体检 A1：近似口径罚款的投诉源之一是
 *  「不知道怎么算」——seed=月内日均做种体积，publish=月内过审发种数）。 */
export function ContractSignBoard({
  tiers,
  dict,
}: {
  tiers: TiersResp;
  dict: Dict;
}) {
  const t = dict.contracts;
  const router = useRouter();
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [okMsg, setOkMsg] = useState<string | null>(null);

  async function sign(kind: string, target: number, autoRenew: boolean) {
    setBusy(`${kind}:${target}`);
    setErr(null);
    setOkMsg(null);
    try {
      await api.post("/api/v1/contracts/sign", {
        kind,
        target,
        auto_renew: autoRenew,
      });
      setOkMsg(t.signedOk);
      router.refresh();
    } catch (e) {
      setErr(e instanceof ApiError ? e.message : t.signFailed);
    } finally {
      setBusy(null);
    }
  }

  const seeds = tiers.tiers.filter((x) => x.kind === "seed");
  const pubs = tiers.tiers.filter((x) => x.kind === "publish");
  const pct = tiers.progress
    ? Math.min(
        100,
        Math.round((tiers.progress.current / tiers.progress.target) * 100),
      )
    : 0;

  return (
    <section className="brand-panel p-4 flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="font-semibold">{t.boardTitle}</h2>
        <p className="text-sm opacity-70">{t.caliber}</p>
      </div>
      {!tiers.enabled && <p className="text-sm">{t.disabled}</p>}
      {tiers.enabled && !tiers.eligible && (
        <p className="text-sm">{t.notEligible}</p>
      )}
      {tiers.progress && (
        <div className="flex flex-col gap-1">
          <div className="flex justify-between text-sm">
            <span>
              {tiers.progress.kind === "publish"
                ? t.progressPublish
                : t.progressSeed}
            </span>
            <span aria-label={t.progressLabel}>
              {tiers.progress.kind === "publish"
                ? tiers.progress.current
                : fmtBytes(tiers.progress.current, dict)}{" "}
              /{" "}
              {tiers.progress.kind === "publish"
                ? tiers.progress.target
                : fmtBytes(tiers.progress.target, dict)}
            </span>
          </div>
          <div
            className="h-2 rounded bg-[var(--surface-sunken)]"
            role="progressbar"
            aria-valuenow={pct}
            aria-valuemin={0}
            aria-valuemax={100}
          >
            <div className="h-2 rounded" style={{ width: `${pct}%` }} />
          </div>
        </div>
      )}
      {err && (
        <p className="text-sm" role="alert">
          {err}
        </p>
      )}
      {okMsg && <p className="text-sm">{okMsg}</p>}
      <div className="grid gap-3 md:grid-cols-3">
        {[
          { list: seeds, title: t.seedTiers },
          { list: pubs, title: t.publishTiers },
        ].map(({ list, title }) => (
          <div key={title} className="flex flex-col gap-2">
            <h3 className="text-sm font-medium">{title}</h3>
            {list.map((x) => (
              <div
                key={`${x.kind}:${x.target}`}
                className="rounded border p-3 flex flex-col gap-2
                  border-[var(--line)]"
              >
                <div className="flex justify-between text-sm font-medium">
                  <span>{x.label}</span>
                  <span>{fmtTarget(x, dict)}</span>
                </div>
                <p className="text-xs opacity-70">
                  {t.reward} +{x.reward} · {t.penalty} -{x.penalty}
                </p>
                <div className="flex gap-2">
                  <button
                    type="button"
                    className={`${BTN_SM_BOLD} min-h-[44px]`}
                    disabled={!tiers.eligible || busy !== null}
                    onClick={() => sign(x.kind, x.target, false)}
                  >
                    {busy === `${x.kind}:${x.target}`
                      ? t.signing
                      : t.sign}
                  </button>
                  <button
                    type="button"
                    className="min-h-[44px] px-3 text-sm"
                    disabled={!tiers.eligible || busy !== null}
                    onClick={() => sign(x.kind, x.target, true)}
                  >
                    {t.signAuto}
                  </button>
                </div>
              </div>
            ))}
          </div>
        ))}
      </div>
    </section>
  );
}

function fmtTarget(x: Tier, _dict: Dict): string {
  return x.kind === "publish" ? `${x.target}` : fmtBytes(x.target, _dict);
}

function fmtBytes(n: number, _dict: Dict): string {
  const gb = 1024 ** 3;
  if (n >= 1024 * gb) return `${(n / (1024 * gb)).toFixed(0)} TB`;
  if (n >= gb) return `${(n / gb).toFixed(0)} GB`;
  return `${n}`;
}

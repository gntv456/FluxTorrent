"use client";

/** 做种收益计算面板（从 page.tsx 按域拆出，300 行门禁）。
 *  后端 /me/spark 已返回 tiers + formula_note，这里只做可视化：
 *  公式 / 官种加成 / 系数参数 / 种子档位 / 时长档位 / 体积与稀有度 /
 *  我的做种分布。不参与任何计费逻辑。 */

import { formatBytes } from "@/lib/format";
import type { Dict } from "@/i18n/zh-CN";
import type { SparkInfo } from "./spark-types";

type MySparkDict = Dict["myspark"];

function Panel({ title, children }: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="brand-panel">
      <div className="brand-panel__head">
        <h2>{title}</h2>
      </div>
      {children}
    </div>
  );
}

function TierTable({
  rows,
  t,
  withCond,
}: {
  rows: { name: string; score: number; cond?: string }[];
  t: MySparkDict;
  withCond?: boolean;
}) {
  return (
    <div className="brand-wide-table-scroll">
      <table className="nexus-table w-full text-xs">
        <thead>
          <tr>
            <th scope="col" className="colhead">{t.colTier}</th>
            <td className="colhead w-16 text-right">
              {t.colScore}
            </td>
            {withCond && (
              <th scope="col" className="colhead">{t.colCond}</th>
            )}
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={i}>
              <td className="rowfollow">{r.name}</td>
              <td className="rowfollow num text-right">
                {r.score}
              </td>
              {withCond && (
                <td className="rowfollow text-sub">{r.cond}</td>
              )}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** 0488 官种加成卡：显眼展示倍率 + 本人官种做种数据，
 *  让「多保官种更划算」这件事用户一眼能看见。 */
function OfficialCard({
  info,
  t,
  mult,
}: {
  info: SparkInfo;
  t: MySparkDict;
  mult: number;
}) {
  return (
    <div className="rounded-[var(--r-md)] border border-line p-3
                    bg-[var(--surface-raised)]">
      <div className="flex flex-wrap items-center gap-3">
        <h3 className="text-sm font-bold">{t.calcOfficialTitle}</h3>
        <span className="pill text-mint">
          {t.calcOfficialMult} ×{mult}
        </span>
        {info.official_seeding_count !== undefined && (
          <span className="pill">
            {t.calcOfficialMy
              .replace("{n}", String(info.official_seeding_count))
              .replace(
                "{size}",
                formatBytes(info.official_seeding_size ?? 0),
              )}
          </span>
        )}
      </div>
      <p className="mt-2 text-xs leading-relaxed text-sub">
        {t.calcOfficialNote.replace("{mult}", String(mult))}
      </p>
      {info.official_seeding_count === 0 && (
        <p className="mt-1 text-xs text-warn">
          {t.calcOfficialMyEmpty}
        </p>
      )}
    </div>
  );
}

export function SeedingCalc({
  info,
  t,
  currency,
}: {
  info: SparkInfo;
  t: MySparkDict;
  currency: string;
}) {
  if (!info.tiers) return null;
  const tiers = info.tiers;
  const f = tiers.formula;
  const params: [string, number][] = [
    [t.paramBase, f.base_per_hour],
    [t.paramCap, f.cap_per_hour],
    [t.paramCurveK, f.curve_k],
    [t.paramScale, f.scale],
    [t.paramDonorMult, f.donor_mult],
    [t.paramOfficialMult, f.official_mult],
  ];
  return (
    <>
      <Panel title={t.ruleTitle}>
        <p className="p-4 text-xs leading-relaxed text-sub">
          {t.rule.replace("{magic}", currency)}
        </p>
      </Panel>

      <Panel title={t.calcTitle}>
        <div className="flex flex-col gap-4 p-4">
          <div>
            <h3 className="mb-1 text-sm font-bold">
              {t.calcFormula}
            </h3>
            <p className="text-xs leading-relaxed text-sub">
              {info.formula_note}
            </p>
          </div>

          {tiers.official && (
            <OfficialCard
              info={info}
              t={t}
              mult={tiers.official.mult}
            />
          )}

          <div>
            <h3 className="mb-1 text-sm font-bold">
              {t.calcParams}
            </h3>
            <div className="brand-wide-table-scroll">
              <table className="nexus-table w-full text-xs">
                <thead>
                  <tr>
                    <th scope="col" className="colhead">{t.colParam}</th>
                    <td className="colhead w-24 text-right">
                      {t.colValue}
                    </td>
                  </tr>
                </thead>
                <tbody>
                  {params.map(([k, v]) => (
                    <tr key={k}>
                      <td className="rowfollow">{k}</td>
                      <td className="rowfollow num text-right">
                        {typeof v === "number" ? v : String(v)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>

          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <div>
              <h3 className="mb-1 text-sm font-bold">
                {t.calcTorrentTier}
              </h3>
              <TierTable
                rows={tiers.torrent_tier}
                t={t}
                withCond
              />
            </div>
            <div>
              <h3 className="mb-1 text-sm font-bold">
                {t.calcSeedHours}
              </h3>
              <TierTable rows={tiers.seed_hours_tier} t={t} />
            </div>
          </div>

          <div>
            <h3 className="mb-1 text-sm font-bold">
              {t.calcVolumeRarity}
            </h3>
            <p className="text-xs leading-relaxed text-sub">
              {tiers.volume.note}
            </p>
            <p className="mt-1 text-xs leading-relaxed text-sub">
              {tiers.rarity.note}
            </p>
          </div>
        </div>
      </Panel>

      {info.reward_rules && info.reward_rules.length > 0 && (
        <Panel title={t.rewardRules}>
          <div className="brand-wide-table-scroll p-4">
            <table className="nexus-table w-full text-xs">
              <thead>
                <tr>
                  <th scope="col" className="colhead">{t.colTier}</th>
                  <td className="colhead w-24 text-right">
                    {t.colCount}
                  </td>
                </tr>
              </thead>
              <tbody>
                {info.reward_rules.map(([name, cnt], i) => (
                  <tr key={i}>
                    <td className="rowfollow">{name}</td>
                    <td className="rowfollow num text-right">
                      {cnt}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Panel>
      )}
    </>
  );
}

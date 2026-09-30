"use client";

/** 娱乐屋运营面板（staff 只读）：三口径 EV 对照 + 门禁自检 + 参数现值 + 定义一览。
 *  EV 由后端按代码常量 + 设置键现值现场复算 —— 站长改 games_* / arcade_budget_*
 *  后重进本页即见变化，门禁红绿随之更新。 */

import { useCallback, useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";
import { api } from "@/lib/api-client";
import { AdminArcadeCrops } from "./admin-arcade-crops";
import { AdminArcadePool } from "./admin-arcade-pool";
import { AdminArcadeItems } from "./admin-arcade-items";
import { AdminArcadeRewards } from "./admin-arcade-rewards";

interface EvRow {
  name: string;
  ev: number;
  note: string;
}
interface Chk {
  name: string;
  pass: boolean | null;
  why: string;
}
interface Defs {
  season_key: string;
  quests: { code: string; ref: string; target: number; reward: number }[];
  milestones: { code: string; need: number; reward: number }[];
  stub_total: number;
  claims_total: number;
}
interface Overview {
  ev: EvRow[];
  checks: Chk[];
  params: Record<string, number>;
  defs: Defs;
}

const CARD =
  "rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4";

export function AdminArcade() {
  const { dict, currency } = useI18n();
  const t = dict.adminArcade;
  const [d, setD] = useState<Overview | null>(null);

  const load = useCallback(async () => {
    try {
      setD(
        await api.get<Overview>("/api/v1/admin/arcade/overview"),
      );
    } catch {
      setD(null);
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load]);

  if (!d) return <p className="text-sm text-sub">{t.loading}</p>;

  return (
    <section className="flex flex-col gap-4">
      <h2 className="font-display text-lg">{t.title}</h2>

      {/* 奖池写侧：面板改的就是玩法读的那张表，EV 闸在保存前 */}
      <div className={CARD}>
        <AdminArcadePool onChanged={load} />
      </div>

      {/* 物品目录：anchor 只读，改派生价由服务端跨池回查把关 */}
      <div className={CARD}>
        <AdminArcadeItems onChanged={load} />
      </div>

      {/* 确定侧奖励（周常 / 赛季）：0247 起从代码常量搬进行表 */}
      <div className={CARD}>
        <AdminArcadeRewards onChanged={load} />
      </div>

      {/* 作物表：一行改动同时决定这一档的回收期望与彩蛋池的定标单位 */}
      <div className={CARD}>
        <AdminArcadeCrops onChanged={load} />
      </div>

      {/* 三口径 EV 对照 */}
      <div className={CARD}>
        <h3 className="mb-2 text-sm font-bold">{t.evTitle}</h3>
        <table className="nexus-table w-full text-xs">
          <tbody>
            {d.ev.map((r) => (
              <tr key={r.name}>
                <td className="font-bold">{r.name}</td>
                <td className="num">
                  <span
                    className={
                      r.ev < 1 ? "text-[var(--mint)]" : "text-[var(--coral)]"
                    }
                  >
                    {r.ev.toFixed(3)}
                  </span>
                  <span className="text-sub">
                    {" "}
                    · {r.ev < 1 ? t.safe : t.risk}
                  </span>
                </td>
                <td className="text-sub">{r.note}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* 门禁自检 */}
      <div className={CARD}>
        <h3 className="mb-2 text-sm font-bold">{t.checksTitle}</h3>
        <div className="flex flex-col gap-1 text-xs">
          {d.checks.map((c) => (
            <div key={c.name} className="flex items-baseline gap-2">
              <span
                className={
                  "inline-block h-2 w-2 flex-none rounded-full " +
                  (c.pass === false
                    ? "bg-[var(--coral)]"
                    : "bg-[var(--mint)]")
                }
              />
              <span>
                {c.name}
                {c.why && <span className="text-sub"> · {c.why}</span>}
              </span>
            </div>
          ))}
        </div>
      </div>

      {/* 参数现值 */}
      <div className={CARD}>
        <h3 className="mb-2 text-sm font-bold">{t.paramsTitle}</h3>
        <dl className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs sm:grid-cols-3">
          {Object.entries(d.params).map(([k, v]) => (
            <div key={k} className="flex justify-between gap-2">
              <dt className="num text-sub">{k}</dt>
              <dd className="num font-bold">{v}</dd>
            </div>
          ))}
        </dl>
      </div>

      {/* 定义一览 */}
      <div className={CARD}>
        <h3 className="mb-2 text-sm font-bold">{t.defsTitle}</h3>
        <p className="text-xs text-sub">
          {t.defQuests}：{d.defs.quests.length} · {t.defMilestones}：
          {d.defs.milestones.length}（{d.defs.season_key}）· {t.defStubs}：
          {d.defs.stub_total} · {t.defClaims}：{d.defs.claims_total}
        </p>
        <p className="mt-1 text-[11px] text-sub">
          {t.defsNote.replace("{magic}", currency)}
        </p>
      </div>
    </section>
  );
}

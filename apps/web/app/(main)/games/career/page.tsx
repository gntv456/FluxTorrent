import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";
import { requireModule } from "@/components/module-gate";
import { PANEL_LG } from "@/lib/ui-classes";

/**
 * 我的游戏生涯（波3 留存层）：总场次/总下注/总回收/净收/最大单赢 +
 * 各玩法分布。全从 spark_ledger(kind='game') 聚合（GET /games/career，
 * 与写侧同一本账），不建新表。
 */

interface CareerGame {
  game: string;
  plays: number;
  wagered: number;
  returned: number;
}

interface Career {
  plays: number;
  wagered: number;
  returned: number;
  net: number;
  best_win: number;
  games: CareerGame[];
}

/** 玩法中文名（i18n 有段的用段 title，其余兜底原 key） */
function gameName(dict: Record<string, { title?: string }>, key: string) {
  return dict[key]?.title ?? key;
}

export default async function CareerPage() {
  const gate = await requireModule("games");
  if (gate) return gate;
  const { dict, locale, currency } = await getDict();
  const t = dict.games;
  const career = await api
    .get<Career>("/api/v1/games/career")
    .catch(() => null);

  const fmt = (n: number) => n.toLocaleString(dateLocale(locale));
  const stats: { lb: string; vl: string; tone?: string }[] = career
    ? [
        { lb: t.careerPlays, vl: fmt(career.plays) },
        { lb: t.careerWagered, vl: `${fmt(career.wagered)} ${currency}` },
        {
          lb: t.careerNet,
          vl: `${career.net > 0 ? "+" : ""}${fmt(career.net)} ${currency}`,
          tone:
            career.net > 0
              ? "text-mint"
              : career.net < 0
                ? "text-[var(--danger-text)]"
                : "text-sub",
        },
        { lb: t.careerBest, vl: `${fmt(career.best_win)} ${currency}` },
      ]
    : [];

  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">Arcade</div>
          <h1 className="font-display text-2xl">{t.careerTitle}</h1>
        </div>
        <Link href="/games" className="text-xs text-[var(--sky-deep)]">
          ← {t.back}
        </Link>
      </div>

      {!career || career.plays === 0 ? (
        <p className={PANEL_LG}>
          {t.careerEmpty.replace("{magic}", currency)}
        </p>
      ) : (
        <>
          <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
            {stats.map((s) => (
              <div key={s.lb} className={PANEL_LG}>
                <div className="text-xs text-sub">{s.lb}</div>
                <b className={`num text-lg ${s.tone ?? "text-ink"}`}>
                  {s.vl}
                </b>
              </div>
            ))}
          </div>
          <section className={PANEL_LG}>
            <h2 className="mb-3 font-display text-base">
              {t.careerByGame}
            </h2>
            <table className="w-full text-xs">
              <thead>
                <tr className="text-left text-sub">
                  <th className="py-1">{t.careerGameCol}</th>
                  <th className="py-1 text-right">{t.careerPlaysCol}</th>
                  <th className="py-1 text-right">{t.careerWageredCol}</th>
                  <th className="py-1 text-right">{t.careerNetCol}</th>
                </tr>
              </thead>
              <tbody>
                {career.games.map((g) => {
                  const net = g.returned - g.wagered;
                  return (
                    <tr key={g.game} className="border-t border-line">
                      <td className="py-1.5 font-bold">
                        {gameName(
                          dict.games as unknown as Record<
                            string,
                            { title?: string }
                          >,
                          g.game,
                        )}
                      </td>
                      <td className="num py-1.5 text-right">
                        {fmt(g.plays)}
                      </td>
                      <td className="num py-1.5 text-right">
                        {fmt(g.wagered)}
                      </td>
                      <td
                        className={`num py-1.5 text-right ${
                          net > 0
                            ? "text-mint"
                            : net < 0
                              ? "text-[var(--danger-text)]"
                              : "text-sub"
                        }`}
                      >
                        {net > 0 ? "+" : ""}
                        {fmt(net)}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
            <p className="mt-3 text-xs text-sub">{t.careerNote}</p>
          </section>
        </>
      )}
    </div>
  );
}

import type { Dict } from "@/i18n/zh-CN";

interface MineRow {
  id: number;
  kind: string;
  month: string;
  target: number;
  achieved: number;
  status: string;
  reward: number;
  penalty: number;
  auto_renew: boolean;
  missed_streak: number;
}

/** 我的契约历史表（server component：纯展示，无交互态）。 */
export function ContractHistory({
  rows,
  dict,
}: {
  rows: MineRow[];
  dict: Dict;
}) {
  const t = dict.contracts;
  return (
    <section className="brand-panel p-4">
      <h2 className="font-semibold mb-3">{t.historyTitle}</h2>
      {rows.length === 0 ? (
        <p className="text-sm opacity-70">{t.historyEmpty}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left opacity-70">
                <th className="py-2 pr-3">{t.colMonth}</th>
                <th className="py-2 pr-3">{t.colKind}</th>
                <th className="py-2 pr-3">{t.colTarget}</th>
                <th className="py-2 pr-3">{t.colAchieved}</th>
                <th className="py-2 pr-3">{t.colStatus}</th>
                <th className="py-2 pr-3">{t.colReward}</th>
                <th className="py-2 pr-3">{t.colAutoRenew}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.id} className="border-t border-[var(--line)]">
                  <td className="py-2 pr-3">{r.month.slice(0, 7)}</td>
                  <td className="py-2 pr-3">
                    {r.kind === "publish" ? t.kindPublish : t.kindSeed}
                  </td>
                  <td className="py-2 pr-3">
                    {r.kind === "publish"
                      ? r.target
                      : fmtSize(r.target)}
                  </td>
                  <td className="py-2 pr-3">
                    {r.status === "open"
                      ? "—"
                      : r.kind === "publish"
                        ? r.achieved
                        : fmtSize(r.achieved)}
                  </td>
                  <td className="py-2 pr-3">{statusLabel(r.status, t)}</td>
                  <td className="py-2 pr-3">
                    {r.reward > 0 && `+${r.reward}`}
                    {r.penalty > 0 && `-${r.penalty}`}
                    {r.reward === 0 && r.penalty === 0 && "—"}
                  </td>
                  <td className="py-2 pr-3">{r.auto_renew ? "✓" : "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

function statusLabel(s: string, t: Dict["contracts"]): string {
  if (s === "settled") return t.stSettled;
  if (s === "breached") return t.stBreached;
  return t.stOpen;
}

function fmtSize(n: number): string {
  const gb = 1024 ** 3;
  if (n >= 1024 * gb) return `${(n / (1024 * gb)).toFixed(1)} TB`;
  if (n >= gb) return `${(n / gb).toFixed(0)} GB`;
  return `${n}`;
}

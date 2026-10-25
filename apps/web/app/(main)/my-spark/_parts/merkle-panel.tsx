"use client";

import { useCallback, useEffect, useState } from "react";

import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import { INPUT_CARD, BTN_SM_BOLD, BTN_SM_GHOST } from "@/lib/ui-classes";
import { verifyLeaf, type ProofStep } from "@/lib/merkle";
import { StatePanel } from "@/components/ui/state-panel";
import { SkeletonRows } from "@/components/ui/skeleton";

interface Row {
  idx: number;
  ledger_id: number;
  amount: number;
  kind: string;
  created_at: string;
  leaf: string;
  preimage: string;
  proof: ProofStep[] | null;
}

interface Manifest {
  built: boolean;
  date: string;
  timezone: string;
  algorithm: string;
  anchoring: string;
  root?: string;
  leaf_count?: number;
  user_count?: number;
  built_at?: string;
  included?: boolean;
  rows: Row[];
}

interface Verdict {
  ok: boolean;
  n: number;
}

/** 每日结算凭证：日期选择 + 本人叶子 + 浏览器现场复算。 */
export function MerklePanel() {
  const { dict, locale } = useI18n();
  const t = dict.myspark;
  const [data, setData] = useState<Manifest | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [date, setDate] = useState("");
  const [busy, setBusy] = useState(false);
  const [verdict, setVerdict] = useState<Verdict | null>(null);
  const [unsupported, setUnsupported] = useState(false);

  const load = useCallback(async (day?: string) => {
    setErr(null);
    setData(null);
    setVerdict(null);
    setUnsupported(false);
    try {
      const q = day ? `?date=${encodeURIComponent(day)}` : "";
      const m = await api.get<Manifest>(
        `/api/v1/me/spark/ledger/manifest${q}`,
      );
      setData(m);
      setDate(m.date);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const verify = async () => {
    const root = data?.root;
    if (!root) return;
    setBusy(true);
    setVerdict(null);
    try {
      let ok = true;
      for (const r of data.rows) {
        if (
          !r.proof ||
          !(await verifyLeaf(r.preimage, r.leaf, r.proof, root))
        ) {
          ok = false;
          break;
        }
      }
      setVerdict({ ok, n: data.rows.length });
    } catch {
      setUnsupported(true);
    } finally {
      setBusy(false);
    }
  };

  const resultLine = verdict
    ? (verdict.ok ? t.merkleOk : t.merkleBad).replace(
        "{n}",
        String(verdict.n),
      )
    : null;

  return (
    <div className="brand-panel">
      <div className="brand-panel__head">
        <h2>{t.merkleTitle}</h2>
        <span className="text-xs text-sub">
          {data?.algorithm ?? "sha256-merkle-v1"}
        </span>
      </div>
      <div className="flex flex-col gap-3 p-4">
        <p className="text-xs leading-relaxed text-sub">{t.merkleDesc}</p>
        <div className="flex flex-wrap items-end gap-2">
          <label className="flex flex-col gap-1 text-xs text-sub">
            {t.merkleDate}
            <input
              type="date"
              value={date}
              onChange={(e) => setDate(e.target.value)}
              className={`${INPUT_CARD} px-2 py-1 text-xs text-ink`}
            />
          </label>
          <button
            type="button"
            className={BTN_SM_GHOST}
            onClick={() => void load(date)}
          >
            {t.merkleLoad}
          </button>
          {data?.built && data.rows.length > 0 && (
            <button
              type="button"
              className={BTN_SM_BOLD}
              disabled={busy}
              onClick={() => void verify()}
            >
              {busy ? t.merkleChecking : t.merkleVerify}
            </button>
          )}
        </div>

        {unsupported && (
          <p className="text-xs text-warn">{t.merkleUnsupported}</p>
        )}
        {resultLine && (
          <p
            className={`text-xs font-bold ${
              verdict?.ok ? "text-success" : "text-danger"
            }`}
          >
            {resultLine}
          </p>
        )}

        <StatePanel
          loading={data === null && err === null}
          error={err}
          empty={data?.built === false}
          emptyTitle={t.merkleNotBuilt}
          loadingFallback={<SkeletonRows rows={3} />}
          onRetry={() => void load(date)}
        >
          {data?.built && (
            <div className="flex flex-col gap-2">
              <p className="text-xs text-sub">
                {t.merkleRoot}{" "}
                <code className="break-all text-ink">{data.root}</code>
              </p>
              <p className="text-xs text-sub">
                {t.merkleLeaves
                  .replace("{n}", String(data.leaf_count ?? 0))
                  .replace("{u}", String(data.user_count ?? 0))}
                {data.built_at && (
                  <>
                    {" · "}
                    {t.merkleBuiltAt}{" "}
                    {new Date(data.built_at).toLocaleString(
                      dateLocale(locale),
                    )}
                  </>
                )}
              </p>
              {data.rows.length === 0 ? (
                <p className="text-xs text-sub">{t.merkleNoRows}</p>
              ) : (
                <div className="brand-wide-table-scroll">
                  <table className="nexus-table profilet">
                    <tbody>
                      <tr>
                        <td className="colhead">{t.ledgerAt}</td>
                        <td className="colhead">{t.ledgerKind}</td>
                        <td className="colhead">{t.ledgerAmount}</td>
                        <td className="colhead">{t.merkleLeaf}</td>
                      </tr>
                      {data.rows.map((r) => (
                        <tr key={r.idx}>
                          <td className="text-xs text-sub">
                            {new Date(r.created_at).toLocaleString(
                              dateLocale(locale),
                            )}
                          </td>
                          <td>
                            <span className="pill">
                              {dict.my.kinds[r.kind] ?? t.otherKind}
                            </span>
                          </td>
                          <td
                            className={`num font-bold ${
                              r.amount >= 0
                                ? "text-success"
                                : "text-danger"
                            }`}
                          >
                            {r.amount >= 0 ? "+" : ""}
                            {r.amount.toLocaleString()}
                          </td>
                          <td className="text-xs text-sub">
                            <code title={r.leaf}>
                              {r.leaf.slice(0, 12)}…
                            </code>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
              <p className="text-xs leading-relaxed text-sub">
                {t.merkleNote}
              </p>
            </div>
          )}
        </StatePanel>
      </div>
    </div>
  );
}

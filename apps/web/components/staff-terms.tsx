"use client";

/** 术语表面板（0205 / 四审 L7 的「方向盘」）：
 *  站长把「种子 / 魔力 / 保种」这类写死的固有词改成本站叫法。
 *
 *  面板只管规则本身；改写发生在两个出口（前端字典出口 `i18n/server.ts` +
 *  后端错误信封 `errors.rs`），所以这里绝不重复实现一遍替换语义——
 *  重复实现就会有第二份真值（本仓反复踩过的形态）。
 *
 *  底部「试替换」复用出口上同一个 `applyTermsText`，因此它显示的就是线上结果。
 *  整本字典的命中数**不在这里算**（面板拿到的 dict 已改写，数出来恒为 0 是假绿），
 *  那条「登记了却零命中」的检查由 `scripts/terms_guard.mjs` 在 CI 里扫字典源文件。 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { applyTermsText, sortRules } from "@/i18n/apply-terms";

interface TermRow {
  canonical: string;
  replacement: string;
  enabled: boolean;
  descr: string;
  sort: number;
}

interface Draft {
  canonical: string;
  replacement: string;
  sort: number;
}

const FLD =
  "min-h-[38px] w-full rounded-[var(--r-sm)] border border-line bg-cloud " +
  "px-2 text-sm outline-none focus:border-sky";

const BTN_PRIMARY =
  "rounded-full bg-sky px-4 py-1.5 text-sm font-bold text-white " +
  "disabled:opacity-50";
const BTN_GHOST =
  "rounded-full border border-line px-4 py-1.5 text-sm font-bold";

const emptyDraft = (): Draft => ({
  canonical: "",
  replacement: "",
  sort: 100,
});

export function StaffTermsPanel({
  flash,
}: {
  flash: (m: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.termsPanel;
  const [rows, setRows] = useState<TermRow[] | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [busy, setBusy] = useState(false);
  const [probe, setProbe] = useState("");

  const load = useCallback(() => {
    api
      .get<TermRow[]>("/api/v1/admin/terms")
      .then(setRows)
      .catch(() => setRows([]));
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  function errText(e: unknown) {
    return e instanceof ApiError ? e.message : dict.common.networkError;
  }

  /** 启用的规则（试替换用）。顺序由 sortRules 保证长词优先。 */
  const live = (rows ?? []).filter((r) => r.enabled);
  const rules = sortRules(
    live.map((r) => ({
      canonical: r.canonical,
      replacement: r.replacement,
    })),
  );

  async function save() {
    if (!draft || busy) return;
    setBusy(true);
    try {
      await api.post("/api/v1/admin/terms", {
        canonical: draft.canonical.trim(),
        replacement: draft.replacement.trim(),
        enabled: true,
        sort: draft.sort,
      });
      flash(t.saved);
      setDraft(null);
      load();
    } catch (e) {
      flash(errText(e));
    } finally {
      setBusy(false);
    }
  }

  async function toggle(r: TermRow) {
    try {
      await api.post(
        `/api/v1/admin/terms/${encodeURIComponent(r.canonical)}/toggle`,
        {},
      );
      load();
    } catch (e) {
      flash(errText(e));
    }
  }

  async function remove(r: TermRow) {
    if (!window.confirm(fmt(t.delConfirm, { w: r.canonical }))) return;
    try {
      await api.post(
        `/api/v1/admin/terms/${encodeURIComponent(r.canonical)}/delete`,
        { confirm: true },
      );
      flash(t.deleted);
      load();
    } catch (e) {
      flash(errText(e));
    }
  }

  const editForm = draft && (
    <div className="grid gap-2 rounded-[var(--r-md)] border border-line p-3">
      <div className="grid gap-2 md:grid-cols-3">
        <label className="text-xs text-sub">
          {t.canonical}
          <input
            className={FLD}
            value={draft.canonical}
            onChange={(e) => setDraft({ ...draft, canonical: e.target.value })}
          />
        </label>
        <label className="text-xs text-sub">
          {t.replacement}
          <input
            className={FLD}
            value={draft.replacement}
            onChange={(e) =>
              setDraft({ ...draft, replacement: e.target.value })
            }
          />
        </label>
        <label className="text-xs text-sub">
          {t.sort}
          <input
            type="number"
            className={FLD}
            value={draft.sort}
            onChange={(e) =>
              setDraft({ ...draft, sort: Number(e.target.value) })
            }
          />
        </label>
      </div>
      <div className="flex gap-2">
        <button
          type="button"
          className={BTN_PRIMARY}
          onClick={save}
          disabled={
            busy ||
            !draft.canonical.trim() ||
            !draft.replacement.trim() ||
            draft.canonical.trim() === draft.replacement.trim()
          }
        >
          {dict.usercp.saveBtn}
        </button>
        <button
          type="button"
          className="rounded-full border border-line px-4 py-1.5 text-sm"
          onClick={() => setDraft(null)}
        >
          {dict.common.cancel}
        </button>
      </div>
    </div>
  );

  const tried = probe.trim() ? applyTermsText(probe, rules) : "";

  return (
    <div className="flex flex-col gap-3">
      <p className="text-xs text-sub">{t.hint}</p>
      <p className="text-xs text-sub">{t.how}</p>
      <div className="flex items-center justify-between gap-3">
        <span className="text-xs text-sub">
          {fmt(t.count, { n: live.length, m: (rows ?? []).length })}
        </span>
        {!draft && (
          <button
            type="button"
            className={BTN_GHOST}
            onClick={() => setDraft(emptyDraft())}
          >
            + {t.add}
          </button>
        )}
      </div>
      {editForm}
      <table className="nexus-table">
        <thead>
          <tr>
            <th>{t.canonical}</th>
            <th>{t.replacement}</th>
            <th>{t.status}</th>
            <th>{t.sort}</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {(rows ?? []).map((r) => (
            <tr key={r.canonical}>
              <td>{r.canonical}</td>
              <td>{r.replacement}</td>
              <td className="text-xs">{r.enabled ? t.on : t.off}</td>
              <td className="num text-xs">{r.sort}</td>
              <td className="flex gap-2 text-xs">
                <button
                  type="button"
                  className="underline"
                  onClick={() =>
                    setDraft({
                      canonical: r.canonical,
                      replacement: r.replacement,
                      sort: r.sort,
                    })
                  }
                >
                  {dict.torrents.edit}
                </button>
                <button
                  type="button"
                  className="underline"
                  onClick={() => toggle(r)}
                >
                  {r.enabled ? t.pause : t.resume}
                </button>
                <button
                  type="button"
                  className="underline text-coral"
                  onClick={() => remove(r)}
                >
                  {dict.torrents.delete}
                </button>
              </td>
            </tr>
          ))}
          {rows !== null && rows.length === 0 && (
            <tr>
              <td colSpan={5} className="text-sub">
                {t.empty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
      <div className="grid gap-1 rounded-[var(--r-md)] border border-line p-3">
        <label className="text-xs text-sub">
          {t.tryIt}
          <input
            className={FLD}
            value={probe}
            onChange={(e) => setProbe(e.target.value)}
            placeholder={t.tryHint}
          />
        </label>
        {probe.trim() !== "" && (
          <p className="text-xs">
            {tried === probe ? (
              <span className="text-sub">{t.trySame}</span>
            ) : (
              <>
                <span className="text-sub line-through">{probe}</span>
                {" → "}
                <strong>{tried}</strong>
              </>
            )}
          </p>
        )}
      </div>
    </div>
  );
}

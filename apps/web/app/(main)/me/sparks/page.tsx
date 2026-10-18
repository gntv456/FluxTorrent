"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt, fmtCur } from "@/i18n/config";
import { SPARK_KIND_LABELS } from "@/lib/spark-kind-labels";
import { PageHeader } from "@/components/ui/page-header";
import { FilterLinks } from "@/components/ui/filter-links";
import { Pager } from "@/components/ui/pager";
import { SkeletonRows } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/ui/empty-state";
import { INPUT_CARD } from "@/lib/ui-classes";

/** C5 用户侧「火花流水」页（0226，触点 #11）：
 *  消费 GET /me/spark/ledger 的分页信封（2026-10-17 起带 kind/dir 筛选）。
 *  kind 的人话映射与后台 SparkPanel 共享（lib/spark-kind-labels）。
 *
 *  2026-10-17 一并做的三件事：整页文案从硬编码中文收进 `dict.ledger`
 *  （此前 `void dict;` 把字典取来又丢掉）；补上样图那三档筛选与摘要列；
 *  取档失败不再演成「暂无流水」。 */

interface LedgerEnvelope {
  items: LedgerRow[];
  total: number;
  page: number;
  per: number;
}

interface LedgerRow {
  amount: number;
  kind: string;
  balance_after: number | null;
  created_at: string;
  item_name?: string | null;
  ref_type?: string | null;
  ref_id?: number | null;
}

export default function MySparksPage() {
  const { dict, locale, currency } = useI18n();
  const t = dict.ledger;
  const [data, setData] = useState<LedgerEnvelope | null>(null);
  const [failed, setFailed] = useState(false);
  const [page, setPage] = useState(1);
  const [dir, setDir] = useState("");
  const [kind, setKind] = useState("");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [balance, setBalance] = useState<number | null>(null);

  useEffect(() => {
    api
      .get<{ balance: number }>("/api/v1/me/spark")
      .then((d) => setBalance(d.balance))
      .catch(() => setBalance(null));
  }, []);

  // 后端谓词是 created_at >= from AND created_at < to（repo/spark_ledger.rs:59-60），
  // 所以「止」那一天要送**次日零点**才包含整天；本地日界用 toISOString 折成 UTC。
  const dayEdge = (v: string, plusDay: boolean) => {
    const d = new Date(`${v}T00:00:00`);
    if (Number.isNaN(d.getTime())) return "";
    if (plusDay) d.setDate(d.getDate() + 1);
    return d.toISOString();
  };

  const load = useCallback(() => {
    const sp = new URLSearchParams({ page: String(page) });
    if (dir) sp.set("dir", dir);
    if (kind) sp.set("kind", kind);
    if (from) sp.set("from", dayEdge(from, false));
    if (to) sp.set("to", dayEdge(to, true));
    setData(null);
    api
      .get<LedgerEnvelope>(`/api/v1/me/spark/ledger?${sp.toString()}`)
      .then((d) => {
        setData(d);
        setFailed(false);
      })
      .catch(() => setFailed(true));
  }, [page, dir, kind, from, to]);
  useEffect(load, [load]);

  const per = data?.per ?? 50;
  const pages = Math.max(1, Math.ceil((data?.total ?? 0) / per));
  const rows = data?.items ?? [];
  const kindOptions = Object.entries(SPARK_KIND_LABELS).sort((a, b) =>
    a[1].localeCompare(b[1], dateLocale(locale)),
  );
  const summary = (r: LedgerRow) =>
    r.item_name ??
    (r.ref_type ? `${r.ref_type}#${r.ref_id ?? 0}` : t.noSummary);

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow={`${dict.my.title} / ${t.title}`}
        title={fmtCur(t.title, {}, currency)}
        sub={fmtCur(t.sub, {}, currency)}
        aside={
          <span className="sticker">
            {fmt(t.balanceBadge, {
              n: balance === null ? "…" : balance.toLocaleString(),
            })}
          </span>
        }
      />

      <div className="flex flex-wrap items-center gap-2">
        <FilterLinks
          label={t.colDelta}
          value={dir}
          onChange={(v) => {
            setDir(v ?? "");
            setPage(1);
          }}
          options={[
            { label: t.tabAll },
            { label: t.tabIn, value: "in" },
            { label: t.tabOut, value: "out" },
          ]}
        />
        <select
          aria-label={t.colKind}
          value={kind}
          onChange={(e) => {
            setKind(e.target.value);
            setPage(1);
          }}
          className={`${INPUT_CARD} px-2 text-xs text-ink`}
        >
          <option value="">{t.tabAll}</option>
          {kindOptions.map(([v, label]) => (
            <option key={v} value={v}>
              {label}
            </option>
          ))}
        </select>
        {/* 时间范围（样图 § sparks 的「筛选时间范围」）：后端 from/to 早就
            支持（spark_ledger_http.rs:36-40），此前是前端没接。本页所有筛选
            都是改了就重取，故不加样图那颗「筛选」按钮——它会是第二套同义操作。 */}
        <label className="flex items-center gap-1 text-xs text-sub">
          {t.dateFrom}
          <input
            type="date"
            value={from}
            onChange={(e) => {
              setFrom(e.target.value);
              setPage(1);
            }}
            className={`${INPUT_CARD} px-2 py-1 text-xs text-ink`}
          />
        </label>
        <label className="flex items-center gap-1 text-xs text-sub">
          {t.dateTo}
          <input
            type="date"
            value={to}
            max={from || undefined}
            onChange={(e) => {
              setTo(e.target.value);
              setPage(1);
            }}
            className={`${INPUT_CARD} px-2 py-1 text-xs text-ink`}
          />
        </label>
        {(from || to) && (
          <button
            type="button"
            className="text-xs text-sub underline"
            onClick={() => {
              setFrom("");
              setTo("");
              setPage(1);
            }}
          >
            {t.clearRange}
          </button>
        )}
      </div>

      {failed ? (
        <EmptyState
          icon="info"
          tone="warn"
          title={dict.readState.failTitle}
          desc={dict.readState.failDesc}
        />
      ) : data === null ? (
        <SkeletonRows />
      ) : rows.length === 0 ? (
        <EmptyState
          icon="info"
          title={t.empty}
          desc={t.emptyHint}
        />
      ) : (
        <div className="baozi-panel overflow-x-auto">
          <table className="nexus-table">
            <thead>
              <tr>
                <td className="colhead w-32">{t.colTime}</td>
                <td className="colhead w-28">{t.colKind}</td>
                <td className="colhead">{t.colSummary}</td>
                <td className="colhead w-24 text-right">{t.colDelta}</td>
                <td className="colhead w-24 text-right">{t.colBalance}</td>
              </tr>
            </thead>
            <tbody>
              {rows.map((r, i) => (
                <tr key={`${r.created_at}-${i}`}>
                  <td className="num text-xs text-sub">
                    {new Date(r.created_at).toLocaleString(
                      dateLocale(locale),
                    )}
                  </td>
                  <td className="text-xs">
                    {SPARK_KIND_LABELS[r.kind] ?? r.kind}
                  </td>
                  <td className="text-xs text-body">{summary(r)}</td>
                  <td
                    className={`num text-right ${
                      r.amount > 0 ? "text-up" : "text-down"
                    }`}
                  >
                    {r.amount > 0 ? "+" : ""}
                    {r.amount.toLocaleString()}
                  </td>
                  <td className="num text-right">
                    {r.balance_after === null
                      ? "—"
                      : r.balance_after.toLocaleString()}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {!failed && pages > 1 && (
        <Pager
          layout="compact"
          page={page}
          pages={pages}
          posLabel={`${fmt(dict.common.pageX, { n: page })} / ${pages}`}
          total={data?.total ?? 0}
          onStep={setPage}
        />
      )}

      <div className="baozi-panel">
        <div className="baozi-panel__head">
          <h2>{t.noteTitle}</h2>
        </div>
        <p className="p-4 text-xs leading-relaxed text-sub">{t.noteBody}</p>
      </div>
    </div>
  );
}

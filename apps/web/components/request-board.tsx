"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { RequestForm } from "@/components/request-board-form";

interface RequestRow {
  id: number;
  username: string | null;
  title: string;
  descr: string | null;
  bounty: number;
  latest_bounty: number;
  comments: number;
  bids: number;
  status: number;
  fulfilled_torrent_id: number | null;
  created_at: string;
}

/** 求种区列表（参考站 viewrequests.php）：
 *  REQUEST CENTER 头部 + 添加求种/查看所有/已解决/未解决/解决中/我发布的 筛选
 *  + 名称/最新出价/原始出价/评论数/应求数/求种者/时间/状态 八列表格 + 搜索
 *  （发布求种表单拆到 request-board-form.tsx） */
export function RequestBoard({
  initialFinished,
  initialSearch,
}: {
  initialFinished: string;
  initialSearch: string;
}) {
  const { dict } = useI18n();
  const t = dict.requests;
  const [finished, setFinished] = useState(initialFinished);
  const [search, setSearch] = useState(initialSearch);
  const [rows, setRows] = useState<RequestRow[] | null>(null);
  const [loading, setLoading] = useState(false);
  // 发布求种表单（此前整站无 POST /requests 入口，求种业务发不出第一步）
  const [showForm, setShowForm] = useState(false);

  const load = useCallback(async (fin: string, q: string) => {
    setLoading(true);
    try {
      const params = new URLSearchParams({ finished: fin });
      if (q.trim()) params.set("search", q.trim());
      setRows(await api.get<RequestRow[]>(`/api/v1/requests?${params}`));
    } catch {
      setRows([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load(finished, search);
    // 同步到 URL（保持旧站 ?finished= 语义）
    const u = new URL(window.location.href);
    u.searchParams.set("finished", finished);
    if (search.trim()) u.searchParams.set("search", search.trim());
    else u.searchParams.delete("search");
    window.history.replaceState(null, "", u);
  }, [finished, search, load]);

  return (
    <div className="flex flex-col gap-3">
      {/* REQUEST CENTER 头部（参考站 request-center__header 同款斜切装饰） */}
      <header className="request-center__header">
        <span>
          <small>{t.headSmall}</small>
          <strong>{t.title}</strong>
        </span>
      </header>

      {/* 六个筛选入口（第一个为 primary 添加求种） */}
      <nav className="request-center__filters" aria-label={t.title}>
        <button
          type="button"
          className="request-center__filter request-center__filter--primary"
          onClick={() => setShowForm((v) => !v)}
        >
          {t.filterNew}
        </button>
        {(
          [
            ["all", t.filterAll],
            ["yes", t.filterYes],
            ["no", t.filterNo],
            ["ing", t.filterIng],
            ["my", t.filterMy],
          ] as const
        ).map(([key, label]) => (
          <button
            key={key}
            type="button"
            className="request-center__filter"
            data-active={finished === key ? "true" : undefined}
            aria-current={finished === key ? "page" : undefined}
            onClick={() => setFinished(key)}
          >
            {label}
          </button>
        ))}
      </nav>

      {showForm && (
        <RequestForm
          onCreated={() => load(finished, search)}
          onCancel={() => setShowForm(false)}
        />
      )}

      {/* 八列表格 */}
      <section className="request-center__list" aria-label={t.title}>
        <div
          className="baozi-wide-table-scroll"
          role="region"
          aria-label="scrollable table"
        >
          <table className="nexus-table request-center__table">
            <thead>
              <tr>
                <th align="left">{t.colName}</th>
                <th>{t.colLatestBounty}</th>
                <th>{t.colBounty}</th>
                <th>{t.colComments}</th>
                <th>{t.colBids}</th>
                <th>{t.colRequester}</th>
                <th>{t.colTime}</th>
                <th>{t.colStatus}</th>
              </tr>
            </thead>
            <tbody>
              {(rows ?? []).map((r) => (
                <tr key={r.id}>
                  <td>
                    <a
                      className="request-center__name"
                      href={`/requests/${r.id}`}
                    >
                      {r.title}
                    </a>
                    {r.descr && (
                      <p className="request-center__descr">{r.descr}</p>
                    )}
                  </td>
                  <td className="request-center__reward">
                    <strong className="num">
                      {r.latest_bounty.toLocaleString()}
                    </strong>
                  </td>
                  <td className="num">{r.bounty.toLocaleString()}</td>
                  <td className="num">{r.comments}</td>
                  <td className="num">{r.bids}</td>
                  <td>
                    <span className="nowrap">{r.username ?? "—"}</span>
                  </td>
                  <td
                    className="nowrap"
                    title={new Date(r.created_at).toLocaleString("zh-CN")}
                  >
                    {timeAgo(r.created_at)}
                  </td>
                  <td>
                    {r.fulfilled_torrent_id ? (
                      <a
                        className="request-status is-done"
                        href={`/torrent/${r.fulfilled_torrent_id}`}
                      >
                        {t.fulfilled}
                      </a>
                    ) : (
                      <span className="flex items-center gap-1">
                        <span className="request-status is-progress">
                          {t.pending}
                        </span>
                        <button
                          type="button"
                          className="min-h-[28px] rounded-full border border-line px-2 text-[11px] font-bold text-sky-deep"
                          title={t.fulfillTorrentId}
                          onClick={async () => {
                            const v = prompt(t.fulfillTorrentId);
                            const tid = v ? parseInt(v, 10) : NaN;
                            if (!Number.isFinite(tid) || tid <= 0) {
                              if (v !== null) alert(t.fulfillInvalid);
                              return;
                            }
                            try {
                              await api.post("/api/v1/requests/fulfill", {
                                request_id: r.id,
                                torrent_id: tid,
                              });
                              load(finished, search);
                            } catch (e) {
                              alert(
                                t.fulfillFailed +
                                  (e instanceof Error ? e.message : ""),
                              );
                            }
                          }}
                        >
                          {t.fulfillBtn}
                        </button>
                      </span>
                    )}
                  </td>
                </tr>
              ))}
              {rows !== null && rows.length === 0 && (
                <tr>
                  <td colSpan={8} className="py-8 text-center text-sub">
                    {t.empty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </section>

      {/* 搜索 + 分页占位（与旧站底部一致） */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="text-xs text-sub">1 - {rows?.length ?? 0}</span>
        <form
          className="flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            load(finished, search);
          }}
        >
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t.searchPlaceholder}
            className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-3 text-sm"
          />
          <button
            type="submit"
            className="min-h-[40px] rounded-full bg-sky px-4 text-sm font-bold text-white"
            disabled={loading}
          >
            {t.search}
          </button>
        </form>
      </div>
    </div>
  );
}

function timeAgo(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const m = Math.floor(diff / 60000);
  if (m < 60) return `${m}分钟`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}时${m % 60}分`;
  const d = Math.floor(h / 24);
  if (d < 30) return `${d}天${h % 24}时`;
  const mo = Math.floor(d / 30);
  return `${mo}月${d % 30}天`;
}

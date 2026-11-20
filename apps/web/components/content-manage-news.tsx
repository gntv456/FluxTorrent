"use client";

import { BTN_SM_BOLD, CELL_CARD_SUB } from "@/lib/ui-classes";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";

/** 内容管理的公告 tab 面板（0505 从 content-manage.tsx 按域拆出，
 *  300 门禁；连带定时发布字段与 GET /admin/news 全量列表接线）。 */

/// 后台公告列表行（GET /admin/news，0505）：含未到点定时待发条目
export interface AdminNewsRow {
  id: number;
  title: string;
  badge: string;
  created_at: string;
  published_at: string | null;
}

export function NewsTab({
  news,
  load,
}: {
  news: AdminNewsRow[];
  load: () => void | Promise<void>;
}) {
  const { dict } = useI18n();
  const t = dict.cmgmt;
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const ask = useConfirm();
  // 公告编辑器（0505：publishAt 定时发布，空=立即）
  const [nEdit, setNEdit] = useState<{
    id: number | null;
    title: string;
    body: string;
    badge: string;
    publishAt: string;
  }>({
    id: null,
    title: "",
    body: "",
    badge: "公告",
    publishAt: "",
  });

  function flash(m: string) {
    setMsg(m);
    setTimeout(() => setMsg(null), 2500);
  }

  async function saveNews() {
    if (!nEdit.title.trim() || !nEdit.body.trim()) return;
    setBusy(true);
    try {
      // 定时发布（0505）：本地时间转 ISO8601（空=立即发布）
      const publishAt = nEdit.publishAt.trim()
        ? new Date(nEdit.publishAt).toISOString()
        : undefined;
      if (nEdit.id === null) {
        await api.post("/api/v1/admin/news", {
          title: nEdit.title,
          body: nEdit.body,
          badge: nEdit.badge,
          ...(publishAt ? { publish_at: publishAt } : {}),
        });
        flash(t.newsCreated);
      } else {
        await api.put(`/api/v1/admin/news/${nEdit.id}`, {
          title: nEdit.title,
          body: nEdit.body,
          badge: nEdit.badge,
          ...(publishAt ? { publish_at: publishAt } : {}),
        });
        flash(t.newsUpdated);
      }
      setNEdit({
        id: null,
        title: "",
        body: "",
        badge: "公告",
        publishAt: "",
      });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }
  function delNews(id: number) {
    ask.open({
      title: t.confirmDelete,
      tone: "danger",
      run: () => {
        void (async () => {
          try {
            await api.del(`/api/v1/admin/news/${id}`);
            flash(t.newsDeleted);
            await load();
          } catch (e) {
            flash(e instanceof ApiError ? e.message : dict.common.networkError);
          }
        })();
      },
    });
  }

  return (
    <>
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <section className="brand-panel p-4">
        <h2 className="mb-3 text-base font-bold text-ink">
          {nEdit.id === null ? t.newsPublish : t.newsEdit}
        </h2>
        <div className="cmgmt-form">
          <label>
            {t.fldTitle}
            <input
              value={nEdit.title}
              onChange={(e) => setNEdit({ ...nEdit, title: e.target.value })}
            />
          </label>
          <label>
            {t.fldBadge}
            <input
              value={nEdit.badge}
              onChange={(e) => setNEdit({ ...nEdit, badge: e.target.value })}
            />
          </label>
          <label>
            定时发布（留空 = 立即）
            <input
              type="datetime-local"
              value={nEdit.publishAt}
              onChange={(e) =>
                setNEdit({ ...nEdit, publishAt: e.target.value })
              }
            />
            <span className="text-xs text-sub">
              设为未来时间 = 预热待发，到点后才在首页公告区可见
            </span>
          </label>
          <label>
            {t.fldBody}
            <textarea
              rows={6}
              value={nEdit.body}
              onChange={(e) => setNEdit({ ...nEdit, body: e.target.value })}
            />
            <span className="text-xs text-sub">{t.newsVideoHint}</span>
          </label>
          <div className="flex gap-2">
            <button className="brand-button" onClick={saveNews} disabled={busy}>
              {nEdit.id === null ? t.btnPublish : t.btnSave}
            </button>
            {nEdit.id !== null && (
              <button
                className={BTN_SM_BOLD}
                onClick={() =>
                  setNEdit({
                    id: null,
                    title: "",
                    body: "",
                    badge: "公告",
                    publishAt: "",
                  })
                }
              >
                {t.btnCancel}
              </button>
            )}
          </div>
        </div>
      </section>
      <div className="brand-wide-table-scroll">
        <table className="nexus-table">
          <thead>
            <tr>
              <th scope="col" className="colhead">
                {t.fldTitle}
              </th>
              <th scope="col" className="colhead">
                {t.fldBadge}
              </th>
              <th scope="col" className="colhead">
                {t.fldDate}
              </th>
              <th scope="col" className="colhead text-right">
                {t.colActions}
              </th>
            </tr>
          </thead>
          <tbody>
            {news.map((n) => {
              const pending =
                n.published_at != null &&
                new Date(n.published_at).getTime() > Date.now();
              return (
                <tr key={n.id}>
                  <td>
                    {n.title}
                    {pending && (
                      <span
                        className="ml-2 rounded bg-sky-soft px-1.5
                          py-0.5 text-xs text-ink"
                      >
                        待发
                      </span>
                    )}
                  </td>
                  <td>{n.badge}</td>
                  <td className="text-sub">
                    {n.published_at
                      ? new Date(n.published_at).toLocaleString()
                      : ""}
                  </td>
                  <td className="text-right">
                    <button
                      className="cmgmt-act"
                      onClick={() =>
                        setNEdit({
                          id: n.id,
                          title: n.title,
                          body: "",
                          badge: n.badge,
                          publishAt: "",
                        })
                      }
                    >
                      {t.btnEdit}
                    </button>
                    <button
                      className="cmgmt-act cmgmt-act--danger"
                      onClick={() => delNews(n.id)}
                    >
                      {t.btnDelete}
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <AskDialog {...ask} />
    </>
  );
}

/** tab 胶囊行（公告/趣味盒/友链共用，自 content-manage 抽出） */
export function MgmtTabs<K extends string>({
  tab,
  setTab,
  tabs,
}: {
  tab: K;
  setTab: (k: K) => void;
  tabs: [K, string][];
}) {
  return (
    <div className="flex flex-wrap gap-2" role="tablist">
      {tabs.map(([k, label]) => (
        <button
          key={k}
          role="tab"
          aria-selected={tab === k}
          onClick={() => setTab(k)}
          className={`min-h-11 rounded-full px-4 text-sm font-bold ${
            tab === k ? "bg-sky text-[var(--text-on-accent)]" : CELL_CARD_SUB
          }`}
        >
          {label}
        </button>
      ))}
    </div>
  );
}

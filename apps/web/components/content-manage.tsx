"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { FunTab, LinksTab } from "@/components/content-manage-tabs";

interface NewsItem {
  id: number;
  title: string;
  body: string;
  badge: string;
  date: string;
}

interface FunItem {
  id: number;
  username: string | null;
  title: string;
  body: string | null;
  status: string;
  added: string;
  fun_votes: number | null;
  dull_votes: number | null;
  my_vote: string | null;
}

interface LinkItem {
  id: number;
  name: string;
  url: string;
  title: string | null;
  status: string;
  admin_name: string | null;
  email: string | null;
  reason: string | null;
}

type MgmtTab = "news" | "fun" | "links";

/** 内容管理（管理后台扩展）：公告 / 趣味盒 / 友情链接 的发布·编辑·删除·禁止·审核。
 *  趣味盒与友链两个 tab 拆出 content-manage-tabs.tsx（300 门禁）。 */
export function ContentManage({ initialTab }: { initialTab?: MgmtTab }) {
  const { dict } = useI18n();
  const t = dict.cmgmt;
  const [tab, setTab] = useState<MgmtTab>(initialTab ?? "news");
  const [news, setNews] = useState<NewsItem[]>([]);
  const [fun, setFun] = useState<FunItem[]>([]);
  const [links, setLinks] = useState<LinkItem[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // 公告编辑器
  const [nEdit, setNEdit] = useState<{ id: number | null; title: string; body: string; badge: string }>({
    id: null, title: "", body: "", badge: "公告",
  });

  const load = useCallback(async () => {
    try {
      const home = await api.get<{ news: NewsItem[] }>("/api/v1/home");
      setNews(home.news);
      setFun(await api.get<FunItem[]>("/api/v1/fun/items?status=all"));
      setLinks(await api.get<LinkItem[]>("/api/v1/admin/links"));
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.loadFailed);
    }
  }, [dict]);
  useEffect(() => {
    load();
  }, [load]);

  function flash(m: string) {
    setMsg(m);
    setTimeout(() => setMsg(null), 2500);
  }

  // ---------- 公告 ----------
  async function saveNews() {
    if (!nEdit.title.trim() || !nEdit.body.trim()) return;
    setBusy(true);
    try {
      if (nEdit.id === null) {
        await api.post("/api/v1/admin/news", { title: nEdit.title, body: nEdit.body, badge: nEdit.badge });
        flash(t.newsCreated);
      } else {
        await api.put(`/api/v1/admin/news/${nEdit.id}`, { title: nEdit.title, body: nEdit.body, badge: nEdit.badge });
        flash(t.newsUpdated);
      }
      setNEdit({ id: null, title: "", body: "", badge: "公告" });
      load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }
  async function delNews(id: number) {
    if (!confirm(t.confirmDelete)) return;
    try {
      await api.del(`/api/v1/admin/news/${id}`);
      flash(t.newsDeleted);
      load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    }
  }

  const TABS: [MgmtTab, string][] = [
    ["news", t.tabNews],
    ["fun", t.tabFun],
    ["links", t.tabLinks],
  ];

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-2" role="tablist">
        {TABS.map(([k, label]) => (
          <button
            key={k}
            role="tab"
            aria-selected={tab === k}
            onClick={() => setTab(k)}
            className={`min-h-[40px] rounded-full px-4 text-sm font-bold ${
              tab === k ? "bg-sky text-white" : "border border-line bg-[var(--surface-card)] text-sub"
            }`}
          >
            {label}
          </button>
        ))}
      </div>
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}

      {/* 公告管理 */}
      {tab === "news" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">
              {nEdit.id === null ? t.newsPublish : t.newsEdit}
            </h2>
            <div className="cmgmt-form">
              <label>
                {t.fldTitle}
                <input value={nEdit.title} onChange={(e) => setNEdit({ ...nEdit, title: e.target.value })} />
              </label>
              <label>
                {t.fldBadge}
                <input value={nEdit.badge} onChange={(e) => setNEdit({ ...nEdit, badge: e.target.value })} />
              </label>
              <label>
                {t.fldBody}
                <textarea rows={6} value={nEdit.body} onChange={(e) => setNEdit({ ...nEdit, body: e.target.value })} />
              </label>
              <div className="flex gap-2">
                <button className="baozi-button" onClick={saveNews} disabled={busy}>
                  {nEdit.id === null ? t.btnPublish : t.btnSave}
                </button>
                {nEdit.id !== null && (
                  <button
                    className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
                    onClick={() => setNEdit({ id: null, title: "", body: "", badge: "公告" })}
                  >
                    {t.btnCancel}
                  </button>
                )}
              </div>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">{t.fldTitle}</td>
                <td className="colhead">{t.fldBadge}</td>
                <td className="colhead">{t.fldDate}</td>
                <td className="colhead text-right">{t.colActions}</td>
              </tr>
              {news.map((n) => (
                <tr key={n.id}>
                  <td>{n.title}</td>
                  <td>{n.badge}</td>
                  <td className="text-sub">{n.date}</td>
                  <td className="text-right">
                    <button
                      className="cmgmt-act"
                      onClick={() => setNEdit({ id: n.id, title: n.title, body: n.body, badge: n.badge })}
                    >
                      {t.btnEdit}
                    </button>
                    <button className="cmgmt-act cmgmt-act--danger" onClick={() => delNews(n.id)}>
                      {t.btnDelete}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      {/* 趣味盒管理 */}
      {tab === "fun" && (
        <FunTab fun={fun} load={load} t={t} />
      )}

      {/* 友情链接管理 */}
      {tab === "links" && <LinksTab links={links} load={load} t={t} />}
    </div>
  );
}

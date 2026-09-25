"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 自定义页面管理面板（0187 一审 R4.4 的「方向盘」，三审 B-2）：
 *  站长创建/编辑/上下线任意内容页（slug → /p/{slug}），导航挂接走
 *  「导航菜单」面板填 /p/{slug}。body 为 HTML（展示侧 ammonia 消毒）。 */

interface CustomPage {
  id: number;
  slug: string;
  title: string;
  body: string;
  visible: boolean;
  sort: number;
  updated_at: string;
}

const FLD =
  "min-h-[38px] w-full rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm outline-none focus:border-sky";

interface Draft {
  slug: string;
  title: string;
  body: string;
  visible: boolean;
  sort: number;
}

const emptyDraft = (): Draft => ({
  slug: "",
  title: "",
  body: "",
  visible: true,
  sort: 100,
});

export function StaffCustomPagesPanel({
  flash,
}: {
  flash: (m: string) => void;
}) {
  const { dict, locale } = useI18n();
  const zh = locale !== "en";
  const [rows, setRows] = useState<CustomPage[] | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [editId, setEditId] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api
      .get<CustomPage[]>("/api/v1/admin/custom-pages")
      .then(setRows)
      .catch(() => setRows([]));
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  function errText(e: unknown) {
    return e instanceof ApiError ? e.message : dict.common.networkError;
  }

  async function save() {
    if (!draft || busy) return;
    setBusy(true);
    try {
      if (editId) {
        await api.put(`/api/v1/admin/custom-pages/${editId}`, draft);
      } else {
        await api.post("/api/v1/admin/custom-pages", draft);
      }
      flash(dict.adminPages.saved);
      setDraft(null);
      setEditId(null);
      load();
    } catch (e) {
      flash(errText(e));
    } finally {
      setBusy(false);
    }
  }

  async function toggleVisible(p: CustomPage) {
    try {
      await api.put(`/api/v1/admin/custom-pages/${p.id}`, {
        ...p,
        visible: !p.visible,
      });
      load();
    } catch (e) {
      flash(errText(e));
    }
  }

  async function remove(p: CustomPage) {
    if (!window.confirm(zh ? `删除页面「${p.title}」？` : `Delete "${p.title}"?`))
      return;
    try {
      await api.post(`/api/v1/admin/custom-pages/${p.id}/delete`, {});
      flash(dict.adminPages.deleted);
      load();
    } catch (e) {
      flash(errText(e));
    }
  }

  const editForm = draft && (
    <div className="mb-4 grid gap-2 rounded-[var(--r-md)] border border-line p-3">
      <div className="grid gap-2 md:grid-cols-3">
        <label className="text-xs text-sub">
          slug（/p/…）
          <input
            className={FLD}
            value={draft.slug}
            disabled={editId !== null}
            onChange={(e) => setDraft({ ...draft, slug: e.target.value })}
            placeholder="about"
          />
        </label>
        <label className="text-xs text-sub md:col-span-2">
          {dict.adminPages.titleLabel}
          <input
            className={FLD}
            value={draft.title}
            onChange={(e) => setDraft({ ...draft, title: e.target.value })}
          />
        </label>
      </div>
      <label className="text-xs text-sub">
        {dict.adminPages.bodyLabel}
        <textarea
          className={`${FLD} min-h-[160px] font-mono`}
          value={draft.body}
          onChange={(e) => setDraft({ ...draft, body: e.target.value })}
          placeholder="<p>…</p>"
        />
      </label>
      <div className="flex flex-wrap items-center gap-4">
        <label className="flex items-center gap-2 text-xs text-sub">
          <input
            type="checkbox"
            checked={draft.visible}
            onChange={(e) => setDraft({ ...draft, visible: e.target.checked })}
          />
          {dict.adminPages.visible}
        </label>
        <label className="flex items-center gap-2 text-xs text-sub">
          {dict.adminPages.sortLabel}
          <input
            type="number"
            className={`${FLD} w-24`}
            value={draft.sort}
            onChange={(e) => setDraft({ ...draft, sort: Number(e.target.value) })}
          />
        </label>
        <div className="flex gap-2">
          <button
            type="button"
            className="rounded-full bg-sky px-4 py-1.5 text-sm font-bold text-white disabled:opacity-50"
            onClick={save}
            disabled={busy || !draft.slug.trim() || !draft.title.trim()}
          >
            {dict.usercp.saveBtn}
          </button>
          <button
            type="button"
            className="rounded-full border border-line px-4 py-1.5 text-sm"
            onClick={() => {
              setDraft(null);
              setEditId(null);
            }}
          >
            {dict.common.cancel}
          </button>
        </div>
      </div>
    </div>
  );

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <p className="text-xs text-sub">{dict.adminPages.hint}</p>
        {!draft && (
          <button
            type="button"
            className="rounded-full border border-line px-4 py-1.5 text-sm font-bold"
            onClick={() => setDraft(emptyDraft())}
          >
            + {dict.adminPages.add}
          </button>
        )}
      </div>
      {editForm}
      <table className="nexus-table">
        <thead>
          <tr>
            <th>slug</th>
            <th>{dict.adminPages.titleLabel}</th>
            <th>{dict.adminPages.visible}</th>
            <th>{dict.adminPages.sortLabel}</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {(rows ?? []).map((p) => (
            <tr key={p.id}>
              <td>
                <a href={`/p/${p.slug}`} className="underline" target="_blank" rel="noreferrer">
                  /p/{p.slug}
                </a>
              </td>
              <td>{p.title}</td>
              <td className="text-xs">
                {p.visible
                  ? dict.adminPages.visibleOn
                  : dict.adminPages.visibleOff}
              </td>
              <td className="num text-xs">{p.sort}</td>
              <td className="flex gap-2 text-xs">
                <button
                  type="button"
                  className="underline"
                  onClick={() => {
                    setEditId(p.id);
                    setDraft({
                      slug: p.slug,
                      title: p.title,
                      body: p.body,
                      visible: p.visible,
                      sort: p.sort,
                    });
                  }}
                >
                  {dict.torrents.edit}
                </button>
                <button
                  type="button"
                  className="underline"
                  onClick={() => toggleVisible(p)}
                >
                  {p.visible
                    ? dict.adminPages.hideAction
                    : dict.adminPages.showAction}
                </button>
                <button
                  type="button"
                  className="underline text-coral"
                  onClick={() => remove(p)}
                >
                  {dict.torrents.delete}
                </button>
              </td>
            </tr>
          ))}
          {rows !== null && rows.length === 0 && (
            <tr>
              <td colSpan={5} className="text-sub">
                {dict.adminPages.empty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}

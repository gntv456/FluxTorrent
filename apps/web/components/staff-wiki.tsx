"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import {
  type WikiPage,
  type WikiRevision,
  type Draft,
  emptyDraft,
  WIKI_FLD as FLD,
} from "@/components/staff-wiki-shared";

/** Wiki 管理面板（0359）：列表 + 编辑表单 + 版本历史抽屉（只读，回滚不做）。
 *  正文是 BBCode——公开页 renderBBCode 渲染 React 节点，天然免疫 XSS；
 *  与 custom_pages（HTML + ammonia）的关键差异，表单占位符亦按 BBCode 提示。 */


export function StaffWikiPanel({ flash }: { flash: (m: string) => void }) {
  const { dict } = useI18n();
  const [rows, setRows] = useState<WikiPage[] | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [editId, setEditId] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  // 版本历史抽屉：正查看哪页的历史
  const [revFor, setRevFor] = useState<number | null>(null);
  const [revs, setRevs] = useState<WikiRevision[] | null>(null);

  const load = useCallback(() => {
    api
      .get<WikiPage[]>("/api/v1/admin/wiki")
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
        await api.put(`/api/v1/admin/wiki/${editId}`, draft);
      } else {
        await api.post("/api/v1/admin/wiki", draft);
      }
      flash(dict.adminWiki.saved);
      setDraft(null);
      setEditId(null);
      load();
    } catch (e) {
      flash(errText(e));
    } finally {
      setBusy(false);
    }
  }

  async function remove(p: WikiPage) {
    if (!window.confirm(dict.adminWiki.deleteConfirm.replace("{t}", p.title)))
      return;
    try {
      await api.post(`/api/v1/admin/wiki/${p.id}/delete`, {});
      flash(dict.adminWiki.deleted);
      load();
    } catch (e) {
      flash(errText(e));
    }
  }

  async function openRevisions(id: number) {
    setRevFor(id);
    setRevs(null);
    try {
      setRevs(
        await api.get<WikiRevision[]>(`/api/v1/admin/wiki/${id}/revisions`),
      );
    } catch {
      setRevs([]);
    }
  }

  const editForm = draft && (
    <div
      className="mb-4 grid gap-2 rounded-[var(--r-md)] border border-line p-3"
    >
      <div className="grid gap-2 md:grid-cols-3">
        <label className="text-xs text-sub">
          slug（/wiki/…）
          <input
            className={FLD}
            value={draft.slug}
            onChange={(e) => setDraft({ ...draft, slug: e.target.value })}
            placeholder="beginner-guide"
          />
        </label>
        <label className="text-xs text-sub md:col-span-2">
          {dict.adminWiki.titleLabel}
          <input
            className={FLD}
            value={draft.title}
            onChange={(e) => setDraft({ ...draft, title: e.target.value })}
          />
        </label>
      </div>
      <label className="text-xs text-sub">
        {dict.adminWiki.bodyLabel}
        <textarea
          className={`${FLD} min-h-[200px] font-mono`}
          value={draft.body}
          onChange={(e) => setDraft({ ...draft, body: e.target.value })}
          placeholder={dict.adminWiki.bodyPh}
        />
      </label>
      <div className="flex flex-wrap items-center gap-4">
        <label className="flex items-center gap-2 text-xs text-sub">
          <input
            type="checkbox"
            checked={draft.visible}
            onChange={(e) => setDraft({ ...draft, visible: e.target.checked })}
          />
          {dict.adminWiki.visible}
        </label>
        <label className="flex items-center gap-2 text-xs text-sub">
          {dict.adminWiki.sortLabel}
          <input
            type="number"
            className={`${FLD} w-24`}
            value={draft.sort}
            onChange={(e) => setDraft({ ...draft, sort: +e.target.value })}
          />
        </label>
        <div className="flex gap-2">
          <button
            type="button"
            className="rounded-full bg-sky px-4 py-1.5 text-sm \
font-bold text-white disabled:opacity-50"
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

  const revDrawer = revFor !== null && (
    <div className="mb-4 rounded-[var(--r-md)] border border-line p-3">
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-sm font-bold">{dict.adminWiki.revisionsTitle}</h3>
        <button
          type="button"
          className="text-xs underline"
          onClick={() => setRevFor(null)}
        >
          {dict.common.cancel}
        </button>
      </div>
      <ul className="flex max-h-72 flex-col gap-2 overflow-y-auto text-xs">
        {(revs ?? []).map((r) => (
          <li
            key={r.id}
            className="rounded-[var(--r-sm)] border border-line p-2"
          >
            <div className="flex flex-wrap items-center gap-2">
              <span className="font-bold">{r.title}</span>
              <span className="text-sub">
                {r.edited_by_name ?? "-"} ·{" "}
                {r.created_at.slice(0, 16).replace("T", " ")}
              </span>
            </div>
            <pre
              className="mt-1 max-h-24 overflow-y-auto whitespace-pre-wrap \
font-mono text-[11px] text-sub"
            >
              {r.body.slice(0, 500)}
            </pre>
          </li>
        ))}
        {revs !== null && revs.length === 0 && (
          <li className="text-sub">{dict.adminWiki.noRevisions}</li>
        )}
      </ul>
    </div>
  );

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <p className="text-xs text-sub">{dict.adminWiki.hint}</p>
        {!draft && (
          <button
            type="button"
            className="rounded-full border border-line px-4 \
py-1.5 text-sm font-bold"
            onClick={() => {
              setDraft(emptyDraft());
              setEditId(null);
            }}
          >
            + {dict.adminWiki.add}
          </button>
        )}
      </div>
      {editForm}
      {revDrawer}
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table">
          <thead>
            <tr>
              <th>slug</th>
              <th>{dict.adminWiki.titleLabel}</th>
              <th>{dict.adminWiki.visible}</th>
              <th>{dict.adminWiki.sortLabel}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {(rows ?? []).map((p) => (
              <tr key={p.id}>
                <td>
                  <a
                    href={`/wiki/${p.slug}`}
                    className="underline"
                    target="_blank"
                    rel="noreferrer"
                  >
                    /wiki/{p.slug}
                  </a>
                </td>
                <td>{p.title}</td>
                <td className="text-xs">
                  {p.visible
                    ? dict.adminWiki.visibleOn
                    : dict.adminWiki.visibleOff}
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
                    onClick={() => openRevisions(p.id)}
                  >
                    {dict.adminWiki.revisionsBtn}
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
                  {dict.adminWiki.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

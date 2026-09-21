"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 内容管理的趣味盒 / 友链两个 tab 面板（从 content-manage.tsx
 *  按域拆出，300 门禁）。 */

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

/** 趣味盒：编辑器 + 列表（禁言站口径：有趣/无聊投票 + 封禁/恢复） */
export function FunTab({
  fun,
  load,
  t,
}: {
  fun: FunItem[];
  load: () => void;
  t: ReturnType<typeof useI18n>["dict"]["cmgmt"];
}) {
  const [fEdit, setFEdit] = useState<{
    id: number | null;
    title: string;
    body: string;
  }>({
    id: null,
    title: "",
    body: "",
  });

  async function saveFun() {
    if (!fEdit.title.trim()) return;
    try {
      if (fEdit.id === null) {
        await api.post("/api/v1/fun/items", {
          title: fEdit.title,
          body: fEdit.body,
        });
      } else {
        await api.put(`/api/v1/fun/items/${fEdit.id}`, {
          title: fEdit.title,
          body: fEdit.body,
        });
      }
      setFEdit({ id: null, title: "", body: "" });
      load();
    } catch {
      /* flash 由父级统一处理 */
    }
  }

  return (
    <>
      <section className="baozi-panel p-4">
        <h2 className="mb-3 text-base font-bold text-ink">
          {fEdit.id === null ? t.funPublish : t.funEdit}
        </h2>
        <div className="cmgmt-form">
          <label>
            {t.fldTitle}
            <input
              value={fEdit.title}
              onChange={(e) => setFEdit({ ...fEdit, title: e.target.value })}
            />
          </label>
          <label>
            {t.fldBody}
            <textarea
              rows={5}
              value={fEdit.body}
              onChange={(e) => setFEdit({ ...fEdit, body: e.target.value })}
            />
          </label>
          <div className="flex gap-2">
            <button className="baozi-button" onClick={saveFun}>
              {fEdit.id === null ? t.btnPublish : t.btnSave}
            </button>
            {fEdit.id !== null && (
              <button
                className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
                onClick={() => setFEdit({ id: null, title: "", body: "" })}
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
            <td className="colhead">{t.funAuthor}</td>
            <td className="colhead">{t.funVotes}</td>
            <td className="colhead">{t.funStatus}</td>
            <td className="colhead text-right">{t.colActions}</td>
          </tr>
          {fun.map((f) => (
            <tr key={f.id}>
              <td>{f.title}</td>
              <td className="text-sub">{f.username ?? "—"}</td>
              <td className="num">
                😂 {f.fun_votes ?? 0} / 😑 {f.dull_votes ?? 0}
              </td>
              <td>
                <span className={`fun-status fun-status--${f.status}`}>
                  {t.funSt[f.status] ?? f.status}
                </span>
              </td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={() =>
                    setFEdit({ id: f.id, title: f.title, body: f.body ?? "" })
                  }
                >
                  {t.btnEdit}
                </button>
                {f.status !== "banned" ? (
                  <button
                    className="cmgmt-act cmgmt-act--danger"
                    onClick={() =>
                      api
                        .call(
                          `PUT /api/v1/fun/items/${f.id}/status {"status":"banned"}`,
                        )
                        .then(load)
                        .catch(() => {})
                    }
                  >
                    {t.btnBan}
                  </button>
                ) : (
                  <button
                    className="cmgmt-act cmgmt-act--ok"
                    onClick={() =>
                      api
                        .call(
                          `PUT /api/v1/fun/items/${f.id}/status {"status":"normal"}`,
                        )
                        .then(load)
                        .catch(() => {})
                    }
                  >
                    {t.btnUnban}
                  </button>
                )}
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  onClick={() => {
                    if (!confirm(t.confirmDelete)) return;
                    api
                      .call(`DELETE /api/v1/fun/items/${f.id}`)
                      .then(load)
                      .catch(() => {});
                  }}
                >
                  {t.btnDelete}
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}

/** 友情链接：列表 + 审核（通过/隐藏/恢复）+ 删除 */
export function LinksTab({
  links,
  load,
  t,
}: {
  links: LinkItem[];
  load: () => void;
  t: ReturnType<typeof useI18n>["dict"]["cmgmt"];
}) {
  async function reviewLink(
    id: number,
    status: "active" | "hidden" | "pending",
  ) {
    try {
      await api.put(`/api/v1/admin/links/${id}`, { status });
      load();
    } catch {
      /* 略 */
    }
  }
  return (
    <table className="nexus-table">
      <tbody>
        <tr>
          <td className="colhead">{t.linkName}</td>
          <td className="colhead">URL</td>
          <td className="colhead">{t.linkAdmin}</td>
          <td className="colhead">{t.linkEmail}</td>
          <td className="colhead">{t.fldStatus}</td>
          <td className="colhead text-right">{t.colActions}</td>
        </tr>
        {links.map((l) => (
          <tr key={l.id}>
            <td>
              {l.name}
              {l.title && <span className="text-sub"> ({l.title})</span>}
            </td>
            <td>
              <a
                href={l.url}
                target="_blank"
                rel="noreferrer"
                className="text-xs"
              >
                {l.url}
              </a>
            </td>
            <td className="text-sub">{l.admin_name ?? "—"}</td>
            <td className="text-sub">{l.email ?? "—"}</td>
            <td>
              <span className={`link-status link-status--${l.status}`}>
                {t.linkSt[l.status] ?? l.status}
              </span>
            </td>
            <td className="text-right">
              {l.status === "pending" && (
                <button
                  className="cmgmt-act cmgmt-act--ok"
                  onClick={() => reviewLink(l.id, "active")}
                >
                  {t.btnApprove}
                </button>
              )}
              {l.status === "active" && (
                <button
                  className="cmgmt-act"
                  onClick={() => reviewLink(l.id, "hidden")}
                >
                  {t.btnHide}
                </button>
              )}
              {l.status === "hidden" && (
                <button
                  className="cmgmt-act cmgmt-act--ok"
                  onClick={() => reviewLink(l.id, "active")}
                >
                  {t.btnShow}
                </button>
              )}
              <button
                className="cmgmt-act cmgmt-act--danger"
                onClick={() => {
                  if (!confirm(t.confirmDelete)) return;
                  api
                    .del(`/api/v1/admin/links/${l.id}`)
                    .then(load)
                    .catch(() => {});
                }}
              >
                {t.btnDelete}
              </button>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

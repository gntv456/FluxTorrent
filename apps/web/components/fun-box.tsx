"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

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

/** 趣味盒前台（参考站 fun.php 复刻）：
 *  当前条目（😂好笑/😑无聊投票）+ 更多（历史列表）+ 发布（24h 冷却）
 *  + 作者编辑/删除自己的、staff 禁止/恢复 —— 与旧站 fun.php 动作一一对应 */
export function FunBox({ embedded = false }: { embedded?: boolean }) {
  const { dict } = useI18n();
  const t = dict.funbox;
  const [items, setItems] = useState<FunItem[] | null>(null);
  const [showMore, setShowMore] = useState(false);
  const [publishing, setPublishing] = useState(false);
  const [pTitle, setPTitle] = useState("");
  const [pBody, setPBody] = useState("");
  const [editing, setEditing] = useState<{
    id: number;
    title: string;
    body: string;
  } | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setItems(await api.get<FunItem[]>("/api/v1/fun/items"));
    } catch {
      setItems([]);
    }
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  function flash(m: string) {
    setMsg(m);
    setTimeout(() => setMsg(null), 2500);
  }
  async function guard(fn: () => Promise<void>) {
    setBusy(true);
    try {
      await fn();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  const current = items?.[0]; // 最新一条为当前展示
  const history = (items ?? []).slice(1);

  async function vote(funId: number, vote: "fun" | "dull") {
    await guard(async () => {
      await api.post("/api/v1/fun/items/vote", { fun_id: funId, vote });
      flash(vote === "fun" ? t.votedFun : t.votedDull);
      load();
    });
  }
  async function publish() {
    if (!pTitle.trim()) return;
    await guard(async () => {
      await api.post("/api/v1/fun/items", { title: pTitle, body: pBody });
      setPTitle("");
      setPBody("");
      setPublishing(false);
      flash(t.published);
      load();
    });
  }
  async function saveEdit() {
    if (!editing || !editing.title.trim()) return;
    await guard(async () => {
      await api.put(`/api/v1/fun/items/${editing.id}`, {
        title: editing.title,
        body: editing.body,
      });
      setEditing(null);
      flash(t.updated);
      load();
    });
  }
  async function remove(id: number) {
    if (!confirm(t.confirmDelete)) return;
    await guard(async () => {
      await api.del(`/api/v1/fun/items/${id}`);
      flash(t.deleted);
      load();
    });
  }
  async function setStatus(id: number, status: "banned" | "normal") {
    await guard(async () => {
      await api.put(`/api/v1/fun/items/${id}/status`, { status });
      flash(status === "banned" ? t.banned : t.restored);
      load();
    });
  }

  return (
    <section
      className={
        embedded ? "baozi-panel funbox" : "baozi-panel funbox funbox--page"
      }
    >
      <header className="baozi-panel__head">
        <h2>
          <span aria-hidden="true">🎲</span> {t.title}
        </h2>
        <button
          type="button"
          className="baozi-button"
          onClick={() => setPublishing((v) => !v)}
        >
          {publishing ? t.closePublish : t.publish}
        </button>
      </header>

      {msg && <p className="funbox__msg">{msg}</p>}

      {/* 发布表单 */}
      {publishing && (
        <div className="funbox__publish cmgmt-form">
          <label>
            {t.fldTitle}
            <input
              value={pTitle}
              onChange={(e) => setPTitle(e.target.value)}
              maxLength={255}
            />
          </label>
          <label>
            {t.fldBody}
            <textarea
              rows={4}
              value={pBody}
              onChange={(e) => setPBody(e.target.value)}
            />
          </label>
          <p className="funbox__note">{t.cooldownNote}</p>
          <button
            type="button"
            className="baozi-button"
            disabled={busy || !pTitle.trim()}
            onClick={publish}
          >
            {t.submit}
          </button>
        </div>
      )}

      {/* 当前条目 */}
      {!current && items !== null && <p className="funbox__empty">{t.empty}</p>}
      {current && (
        <article className="funbox__current">
          {editing?.id === current.id ? (
            <div className="cmgmt-form">
              <label>
                {t.fldTitle}
                <input
                  value={editing.title}
                  onChange={(e) =>
                    setEditing({ ...editing, title: e.target.value })
                  }
                />
              </label>
              <label>
                {t.fldBody}
                <textarea
                  rows={4}
                  value={editing.body}
                  onChange={(e) =>
                    setEditing({ ...editing, body: e.target.value })
                  }
                />
              </label>
              <div className="flex gap-2">
                <button
                  type="button"
                  className="baozi-button"
                  onClick={saveEdit}
                  disabled={busy}
                >
                  {t.save}
                </button>
                <button
                  type="button"
                  className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
                  onClick={() => setEditing(null)}
                >
                  {dict.cmgmt.btnCancel}
                </button>
              </div>
            </div>
          ) : (
            <>
              <h3>{current.title}</h3>
              {current.body && <p>{current.body}</p>}
              <footer className="funbox__footer">
                <span className="funbox__author">
                  {current.username ?? "—"} ·{" "}
                  {new Date(current.added).toLocaleDateString("zh-CN")}
                </span>
                <div className="funbox__votes">
                  {current.my_vote ? (
                    <span className="funbox__voted">
                      😂 {current.fun_votes ?? 0} · 😑 {current.dull_votes ?? 0}
                    </span>
                  ) : (
                    <>
                      <button
                        type="button"
                        className="funbox__vote funbox__vote--fun"
                        disabled={busy}
                        onClick={() => vote(current.id, "fun")}
                      >
                        😂 {t.voteFun} ({current.fun_votes ?? 0})
                      </button>
                      <button
                        type="button"
                        className="funbox__vote funbox__vote--dull"
                        disabled={busy}
                        onClick={() => vote(current.id, "dull")}
                      >
                        😑 {t.voteDull} ({current.dull_votes ?? 0})
                      </button>
                    </>
                  )}
                </div>
                <div className="funbox__acts">
                  <button
                    type="button"
                    className="cmgmt-act"
                    onClick={() =>
                      setEditing({
                        id: current.id,
                        title: current.title,
                        body: current.body ?? "",
                      })
                    }
                  >
                    {t.edit}
                  </button>
                  {current.status !== "banned" && (
                    <button
                      type="button"
                      className="cmgmt-act cmgmt-act--danger"
                      onClick={() => setStatus(current.id, "banned")}
                    >
                      {t.ban}
                    </button>
                  )}
                  <button
                    type="button"
                    className="cmgmt-act cmgmt-act--danger"
                    onClick={() => remove(current.id)}
                  >
                    {t.delete}
                  </button>
                </div>
              </footer>
            </>
          )}
        </article>
      )}

      {/* 更多（历史列表） */}
      {history.length > 0 && (
        <>
          <button
            type="button"
            className="funbox__more-toggle"
            onClick={() => setShowMore((v) => !v)}
          >
            {showMore ? "▴ " + t.collapse : `▾ ${t.more} (${history.length})`}
          </button>
          {showMore && (
            <ul className="funbox__history">
              {history.map((h) => (
                <li
                  key={h.id}
                  className={h.status === "banned" ? "is-banned" : undefined}
                >
                  <div className="funbox__history-main">
                    <strong>{h.title}</strong>
                    <span className="funbox__history-meta">
                      {h.username ?? "—"} ·{" "}
                      {new Date(h.added).toLocaleDateString("zh-CN")} · 😂{" "}
                      {h.fun_votes ?? 0} / 😑 {h.dull_votes ?? 0}
                      {h.status === "banned" && ` · ${t.bannedTag}`}
                    </span>
                  </div>
                  <div className="funbox__acts">
                    {h.status === "banned" ? (
                      <button
                        type="button"
                        className="cmgmt-act cmgmt-act--ok"
                        onClick={() => setStatus(h.id, "normal")}
                      >
                        {t.restore}
                      </button>
                    ) : (
                      <button
                        type="button"
                        className="cmgmt-act cmgmt-act--danger"
                        onClick={() => setStatus(h.id, "banned")}
                      >
                        {t.ban}
                      </button>
                    )}
                    <button
                      type="button"
                      className="cmgmt-act cmgmt-act--danger"
                      onClick={() => remove(h.id)}
                    >
                      {t.delete}
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </section>
  );
}

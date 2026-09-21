"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

import { MyTickets, type MyTicket } from "./contact-staff-tickets";
import { COLORS, FONTS } from "./contact-staff-tickets";

/** PM 管理组（参考站 contactstaff.php 复刻）：
 *  主题输入 + BBCode 工具栏（B/I/U/URL/IMG/List/QUOTE/Close all + 颜色/字体/字号下拉）
 *  + 正文 textarea + 提交/预览 + 我的工单进度（含「确认关闭」闭环）。
 *  我的工单进度表拆至 ./contact-staff-tickets.tsx（300 门禁）。 */
export function ContactStaff() {
  const { dict } = useI18n();
  const t = dict.contactstaff;
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [preview, setPreview] = useState(false);
  const [tickets, setTickets] = useState<MyTicket[] | null>(null);

  const loadTickets = useCallback(async () => {
    try {
      setTickets(await api.get<MyTicket[]>("/api/v1/me/staffmessages"));
    } catch {
      setTickets([]);
    }
  }, []);

  useEffect(() => {
    void loadTickets();
  }, [loadTickets]);

  async function confirmTicket(id: number) {
    try {
      await api.post("/api/v1/me/staffmessages/confirm", { id });
      await loadTickets();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    }
  }

  function wrap(before: string, after = before) {
    const el = document.getElementById("pm-body") as HTMLTextAreaElement | null;
    if (!el) return;
    const { selectionStart: s, selectionEnd: e, value } = el;
    const next =
      value.slice(0, s) + before + value.slice(s, e) + after + value.slice(e);
    setBody(next);
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(s + before.length, e + before.length);
    });
  }

  async function send() {
    if (!subject.trim() || !body.trim()) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/contactstaff", {
        subject: subject.trim(),
        body: body.trim(),
      });
      setSubject("");
      setBody("");
      setMsg(t.sentOk);
      await loadTickets();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="contactstaff-wrap">
      <header className="contactstaff-head">
        <h1>{t.title}</h1>
        <p>{t.subtitle}</p>
      </header>

      <form
        className="contactstaff-form"
        onSubmit={(e) => {
          e.preventDefault();
          void send();
        }}
      >
        <table className="nexus-table nexus-form">
          <tbody>
            <tr>
              <td className="rowhead">{t.subject}</td>
              <td className="rowfollow">
                <input
                  type="text"
                  className="contactstaff-subject"
                  maxLength={100}
                  value={subject}
                  onChange={(e) => setSubject(e.target.value)}
                />
              </td>
            </tr>
            <tr>
              <td className="rowhead">{t.bodyLabel}</td>
              <td className="rowfollow">
                {/* BBCode 工具栏 */}
                <div
                  className="bbcode-toolbar"
                  role="toolbar"
                  aria-label="BBCode"
                >
                  <button
                    type="button"
                    className="codebuttons"
                    style={{ fontWeight: "bold" }}
                    onClick={() => wrap("[b]", "[/b]")}
                  >
                    B
                  </button>
                  <button
                    type="button"
                    className="codebuttons"
                    style={{ fontStyle: "italic" }}
                    onClick={() => wrap("[i]", "[/i]")}
                  >
                    I
                  </button>
                  <button
                    type="button"
                    className="codebuttons"
                    style={{ textDecoration: "underline" }}
                    onClick={() => wrap("[u]", "[/u]")}
                  >
                    U
                  </button>
                  <button
                    type="button"
                    className="codebuttons"
                    onClick={() => wrap("[url]", "[/url]")}
                  >
                    URL
                  </button>
                  <button
                    type="button"
                    className="codebuttons"
                    onClick={() => wrap("[img]", "[/img]")}
                  >
                    IMG
                  </button>
                  <button
                    type="button"
                    className="codebuttons"
                    onClick={() => wrap("[list][*]", "[/list]")}
                  >
                    List
                  </button>
                  <button
                    type="button"
                    className="codebuttons"
                    onClick={() => wrap("[quote]", "[/quote]")}
                  >
                    QUOTE
                  </button>
                  <select
                    className="codebuttons"
                    aria-label={t.color}
                    defaultValue=""
                    onChange={(e) => {
                      if (e.target.value)
                        wrap(`[color=${e.target.value}]`, "[/color]");
                      e.target.value = "";
                    }}
                  >
                    <option value="">{t.color}</option>
                    {COLORS.map((c) => (
                      <option key={c} value={c}>
                        {c}
                      </option>
                    ))}
                  </select>
                  <select
                    className="codebuttons"
                    aria-label={t.font}
                    defaultValue=""
                    onChange={(e) => {
                      if (e.target.value)
                        wrap(`[font=${e.target.value}]`, "[/font]");
                      e.target.value = "";
                    }}
                  >
                    <option value="">{t.font}</option>
                    {FONTS.map((f) => (
                      <option key={f} value={f}>
                        {f}
                      </option>
                    ))}
                  </select>
                  <select
                    className="codebuttons"
                    aria-label={t.size}
                    defaultValue=""
                    onChange={(e) => {
                      if (e.target.value)
                        wrap(`[size=${e.target.value}]`, "[/size]");
                      e.target.value = "";
                    }}
                  >
                    <option value="">{t.size}</option>
                    {[1, 2, 3, 4, 5, 6, 7].map((s) => (
                      <option key={s} value={s}>
                        {s}
                      </option>
                    ))}
                  </select>
                </div>

                <textarea
                  id="pm-body"
                  className="bbcode"
                  rows={20}
                  value={body}
                  onChange={(e) => setBody(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.ctrlKey && e.key === "Enter") void send();
                  }}
                />

                {preview && (
                  <div className="contactstaff-preview" aria-label={t.preview}>
                    <b>{subject || t.subject}</b>
                    <p>{body}</p>
                  </div>
                )}

                <div className="contactstaff-actions">
                  <input
                    type="submit"
                    className="btn"
                    value={t.submit}
                    disabled={busy}
                  />
                  <input
                    type="button"
                    className="btn2"
                    value={preview ? t.edit : t.preview}
                    onClick={() => setPreview((p) => !p)}
                  />
                  <span className="contactstaff-hint">{t.ctrlEnter}</span>
                </div>
                {msg && <p className="contactstaff-msg">{msg}</p>}
              </td>
            </tr>
          </tbody>
        </table>
      </form>

      <MyTickets tickets={tickets} onConfirm={confirmTicket} />
    </div>
  );
}

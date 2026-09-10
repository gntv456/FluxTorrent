"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 友情链接申请（包子站 linksmanage.php?action=apply 复刻）：rowhead/rowfollow 表格 */
export function LinkApply() {
  const { dict } = useI18n();
  const t = dict.linkapply;
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [title, setTitle] = useState("");
  const [admin, setAdmin] = useState("");
  const [email, setEmail] = useState("");
  const [reason, setReason] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit() {
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/links/apply", { name, url, title, admin, email, reason });
      setMsg(t.ok);
      setName(""); setUrl(""); setTitle(""); setAdmin(""); setEmail(""); setReason("");
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  const fields: [string, string, string, (v: string) => void, string?][] = [
    [t.fldName, name, "", setName, t.noteName],
    [t.fldUrl, url, "", setUrl, t.noteUrl],
    [t.fldTitle, title, "", setTitle, t.noteTitle],
    [t.fldAdmin, admin, "", setAdmin, t.noteAdmin],
    [t.fldEmail, email, "", setEmail, t.noteEmail],
  ];

  return (
    <form
      className="linkapply"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <p className="linkapply__note">{t.required}</p>
      <table className="nexus-table nexus-form">
        <tbody>
          {fields.map(([label, value, , setter, note]) => (
            <tr key={label}>
              <td className="rowhead">
                {label}
                <span className="req-star">*</span>
              </td>
              <td className="rowfollow">
                <input
                  className="uc-input-wide"
                  value={value}
                  onChange={(e) => setter(e.target.value)}
                />
                {note && <br />}
                {note && <span className="uc-note">{note}</span>}
              </td>
            </tr>
          ))}
          <tr>
            <td className="rowhead">
              {t.fldReason}
              <span className="req-star">*</span>
            </td>
            <td className="rowfollow">
              <textarea
                className="uc-textarea"
                rows={10}
                value={reason}
                onChange={(e) => setReason(e.target.value)}
              />
              <br />
              <span className="uc-note">{t.noteReason}</span>
            </td>
          </tr>
          <tr>
            <td className="toolbox" colSpan={2} align="center">
              <input type="submit" className="baozi-button" value={t.submit} disabled={busy} />
              <input
                type="reset"
                className="min-h-[34px] ml-3 rounded-full border-0 bg-[#ffedd0] px-4 text-xs font-bold text-[#6c421f]"
                value={t.reset}
              />
            </td>
          </tr>
        </tbody>
      </table>
      {msg && <p className="linkapply__msg">{msg}</p>}
    </form>
  );
}

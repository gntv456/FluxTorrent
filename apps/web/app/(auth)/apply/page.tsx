"use client";

import { BTN_LG_SKY, INPUT_LG } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";

/** 申请制入站页（0227，P2 触点 #3，UNIT3D application 口径）：
 *  站长开启 application_signup=open 后生效；匿名提交举证，staff 人审，
 *  通过自动发绑定邮箱的邀请码（邮件通知）。通道关闭时整页降级提示。 */

export default function ApplyPage() {
  const { dict } = useI18n();
  const [open, setOpen] = useState<boolean | null>(null);
  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [proofLinks, setProofLinks] = useState("");
  const [reason, setReason] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [okMsg, setOkMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api
      .get<{ application_signup?: string }>("/api/v1/register-mode")
      .then((r) => setOpen(r.application_signup === "open"))
      .catch(() => setOpen(false));
  }, []);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setMsg(null);
    setOkMsg(null);
    try {
      const r = await api.post<{ note: string }>("/api/v1/apply", {
        username: username.trim(),
        email: email.trim(),
        proof_links: proofLinks.trim(),
        reason: reason.trim(),
      });
      setOkMsg(r.note);
    } catch (err) {
      setMsg(err instanceof ApiError ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mx-auto flex max-w-md flex-col gap-5 py-8">
      <h1 className="font-display text-2xl">{dict.apply.title}</h1>
      {open === false ? (
        <>
          <p className="text-sm text-sub">{dict.apply.closedNote}</p>
          <Link href="/login" className="text-sm font-bold text-sky">
            {dict.apply.backToLogin}
          </Link>
        </>
      ) : (
        <form onSubmit={submit} className="flex flex-col gap-3">
          <label className="flex flex-col gap-1">
            <span className="text-sm text-sub">{dict.apply.usernameLabel}</span>
            <input
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              required
              minLength={2}
              maxLength={24}
              className={INPUT_LG}
            />
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-sm text-sub">{dict.apply.emailLabel}</span>
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
              className={INPUT_LG}
            />
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-sm text-sub">{dict.apply.proofLabel}</span>
            <textarea
              value={proofLinks}
              onChange={(e) => setProofLinks(e.target.value)}
              required
              rows={3}
              className={INPUT_LG}
            />
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-sm text-sub">{dict.apply.reasonLabel}</span>
            <textarea
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              required
              minLength={20}
              rows={4}
              className={INPUT_LG}
            />
          </label>
          {msg && (
            <p role="alert" className="text-sm text-danger">
              {msg}
            </p>
          )}
          {okMsg && (
            <p role="status" className="text-sm text-mint">
              {okMsg}
            </p>
          )}
          <button type="submit" disabled={busy} className={BTN_LG_SKY}>
            {busy ? dict.apply.busy : dict.apply.submit}
          </button>
          <p className="text-xs text-sub">{dict.apply.termsNote}</p>
        </form>
      )}
    </div>
  );
}

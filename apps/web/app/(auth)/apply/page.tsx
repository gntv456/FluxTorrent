"use client";

import { BTN_LG_SKY, INPUT_LG } from "@/lib/ui-classes";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";

/** 申请制入站页（0227，P2 触点 #3，UNIT3D application 口径）：
 *  站长开启 application_signup=open 后生效；匿名提交举证，staff 人审，
 *  通过自动发绑定邮箱的邀请码（邮件通知）。通道关闭时整页降级提示。 */

export default function ApplyPage() {
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
      <h1 className="font-display text-2xl">申请入站</h1>
      {open === false ? (
        <>
          <p className="text-sm text-sub">
            本站未开放申请通道。请通过受邀成员获取邀请码注册。
          </p>
          <Link href="/login" className="text-sm font-bold text-sky">
            返回登录
          </Link>
        </>
      ) : (
        <form onSubmit={submit} className="flex flex-col gap-3">
          <label className="flex flex-col gap-1">
            <span className="text-sm text-sub">期望的用户名</span>
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
            <span className="text-sm text-sub">
              邮箱（通过后邀请码发往此处）
            </span>
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
              className={INPUT_LG}
            />
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-sm text-sub">
              佐证链接（至少一条：其他站点个人主页 / 证书记录，每行一条）
            </span>
            <textarea
              value={proofLinks}
              onChange={(e) => setProofLinks(e.target.value)}
              required
              rows={3}
              className={INPUT_LG}
            />
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-sm text-sub">
              申请理由（至少 20 字：你如何了解本站、能带来什么）
            </span>
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
            {busy ? "提交中…" : "提交申请"}
          </button>
          <p className="text-xs text-sub">
            提交即表示同意站规；审核通常 1-3 天，结果邮件通知。
          </p>
        </form>
      )}
    </div>
  );
}

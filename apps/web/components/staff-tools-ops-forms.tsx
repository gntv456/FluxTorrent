"use client";

/**
 * 运营域·批量私信 + 添加用户子面板（从 components/staff-tools-ops.tsx
 * 按域拆出）：staffmess 群发与 adduser 建号表单。动作用父级 guard 上抛。
 */

import { useState } from "react";

import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface OpsFormsProps {
  tab: "staffmess" | "adduser";
  busy: boolean;
  guard: (fn: () => Promise<void>, ok: string) => Promise<void>;
}

export function StaffOpsForms({ tab, busy, guard }: OpsFormsProps) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  const [smSubject, setSmSubject] = useState("");
  const [smBody, setSmBody] = useState("");
  const [smMinClass, setSmMinClass] = useState("");
  const [auName, setAuName] = useState("");
  const [auEmail, setAuEmail] = useState("");
  const [auPass, setAuPass] = useState("");

  return (
    <>
      {/* 批量私信（staffmess） */}
      {tab === "staffmess" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.smNew}</h2>
          <div className="cmgmt-form">
            <label>
              {t.fldSubject}
              <input
                value={smSubject}
                onChange={(e) => setSmSubject(e.target.value)}
              />
            </label>
            <label>
              {t.fldBody}
              <textarea
                rows={5}
                value={smBody}
                onChange={(e) => setSmBody(e.target.value)}
              />
            </label>
            <label>
              {t.smMinClass}
              <select
                value={smMinClass}
                onChange={(e) => setSmMinClass(e.target.value)}
              >
                <option value="">{t.smAllUsers}</option>
                <option value="10">Power User+</option>
                <option value="50">Elite+</option>
                <option value="90">{t.smStaffGroup}</option>
              </select>
            </label>
            <button
              className="baozi-button self-start"
              disabled={busy || !smSubject.trim() || !smBody.trim()}
              onClick={() =>
                guard(async () => {
                  await api.post("/api/v1/admin/staffmess", {
                    subject: smSubject,
                    body: smBody,
                    min_class: smMinClass ? Number(smMinClass) : null,
                  });
                  setSmSubject("");
                  setSmBody("");
                }, t.smSent)
              }
            >
              {t.btnSend}
            </button>
          </div>
        </section>
      )}

      {/* 添加用户（adduser） */}
      {tab === "adduser" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.auNew}</h2>
          <div className="cmgmt-form">
            <label>
              {t.auUsername}
              <input
                value={auName}
                onChange={(e) => setAuName(e.target.value)}
              />
            </label>
            <label>
              {t.auEmail}
              <input
                type="email"
                value={auEmail}
                onChange={(e) => setAuEmail(e.target.value)}
              />
            </label>
            <label>
              {t.auPassword}
              <input
                type="password"
                value={auPass}
                onChange={(e) => setAuPass(e.target.value)}
              />
            </label>
            <button
              className="baozi-button self-start"
              disabled={
                busy ||
                !auName.trim() ||
                !auEmail.includes("@") ||
                auPass.length < 8
              }
              onClick={() =>
                guard(async () => {
                  await api.post("/api/v1/admin/adduser", {
                    username: auName,
                    email: auEmail,
                    password: auPass,
                  });
                  setAuName("");
                  setAuEmail("");
                  setAuPass("");
                }, t.auCreated)
              }
            >
              {t.auBtnCreate}
            </button>
            <p className="text-xs text-sub">{t.auNote}</p>
          </div>
        </section>
      )}
    </>
  );
}

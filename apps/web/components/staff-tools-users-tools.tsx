"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 用户域面板·工具区（从 components/staff-tools-users.tsx 按域拆出）：
 *  重置密码（resetpass）/ 删除被禁用户（deldisabled）/ H&R 赦免（hrpardon）。 */

// 重置密码结果提示条 / 删除被禁用户的警示按钮
const RESET_PASS_CLS =
  "mt-3 rounded-[var(--r-md)] bg-sky-soft p-3 font-mono text-sm text-ink";
const DD_BTN =
  "min-h-[40px] rounded-full bg-[var(--baozi-orange-dark)] px-5 " +
  "text-sm font-bold text-white disabled:opacity-50";

export function ResetPassSection({
  busy,
  guard,
}: {
  busy: boolean;
  guard: (fn: () => Promise<void>, ok: string) => void;
}) {
  const t = useI18n().dict.stafftools;
  const [resetId, setResetId] = useState("");
  const [resetPass, setResetPass] = useState<string | null>(null);

  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-3 text-base font-bold text-ink">{t.rpNew}</h2>
      <div className="cmgmt-form">
        <label>
          {t.warnUserId}
          <input
            value={resetId}
            onChange={(e) => setResetId(e.target.value)}
            placeholder="4"
          />
        </label>
        <button
          className="baozi-button self-start"
          disabled={busy || !resetId.trim()}
          onClick={() =>
            guard(async () => {
              const r = await api.post<{ temp_password: string }>(
                "/api/v1/admin/resetpass",
                { user_id: Number(resetId) },
              );
              setResetPass(r.temp_password);
            }, t.rpDone)
          }
        >
          {t.rpBtn}
        </button>
        <p className="text-xs text-sub">{t.rpNote}</p>
      </div>
      {resetPass && (
        <p className={RESET_PASS_CLS}>
          {t.rpTemp}: {resetPass}
        </p>
      )}
    </section>
  );
}

export function DelDisabledSection({
  busy,
  guard,
  flash,
}: {
  busy: boolean;
  guard: (fn: () => Promise<void>, ok: string) => void;
  flash: (m: string) => void;
}) {
  const t = useI18n().dict.stafftools;

  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-3 text-base font-bold text-ink">{t.ddTitle}</h2>
      <p className="mb-3 text-xs text-sub">{t.ddNote}</p>
      <button
        className={DD_BTN}
        disabled={busy}
        onClick={() => {
          if (!window.confirm(t.ddConfirm)) return;
          void guard(async () => {
            const r = await api.post<{ deleted: number }>(
              "/api/v1/admin/deletedisabled",
            );
            flash(t.ddDone.replace("{n}", String(r.deleted)));
          }, "");
        }}
      >
        {t.ddBtn}
      </button>
    </section>
  );
}

export function HrPardonSection({
  busy,
  guard,
}: {
  busy: boolean;
  guard: (fn: () => Promise<void>, ok: string) => void;
}) {
  const t = useI18n().dict.stafftools;
  const [hpUser, setHpUser] = useState("");
  const [hpTorrent, setHpTorrent] = useState("");
  const [hpNote, setHpNote] = useState("");

  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-3 text-base font-bold text-ink">{t.hpTitle}</h2>
      <div className="cmgmt-form">
        <label>
          {t.hpUser}
          <input
            value={hpUser}
            onChange={(e) => setHpUser(e.target.value.replace(/\D/g, ""))}
            placeholder="4"
          />
        </label>
        <label>
          {t.hpTorrent}
          <input
            value={hpTorrent}
            onChange={(e) => setHpTorrent(e.target.value.replace(/\D/g, ""))}
            placeholder="12"
          />
        </label>
        <label>
          {t.fldReason}
          <input value={hpNote} onChange={(e) => setHpNote(e.target.value)} />
        </label>
        <button
          className="baozi-button self-start"
          disabled={busy || !hpUser || !hpTorrent || !hpNote.trim()}
          onClick={() =>
            guard(async () => {
              await api.post("/api/v1/admin/hr/pardon", {
                user_id: Number(hpUser),
                torrent_id: Number(hpTorrent),
                note: hpNote,
              });
              setHpUser("");
              setHpTorrent("");
              setHpNote("");
            }, t.hpDone)
          }
        >
          {t.hpBtn}
        </button>
        <p className="text-xs text-sub">{t.hpNote}</p>
      </div>
    </section>
  );
}

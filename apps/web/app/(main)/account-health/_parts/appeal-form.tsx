"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import type { AppealCopy } from "@/i18n/account-health";
import { BTN_LG_SKY } from "@/lib/ui-classes";

/** 案件申诉表单（POST /api/v1/cheat-appeals，case_id 强关联）。
 *  提交成功后 refresh 一次 RSC：后端会把该案标成「已有未结申诉」，
 *  表单随之收起、结论出现在下方；本组件在等待刷新期间先显示成功行。 */
export function AppealForm({
  caseId,
  copy,
}: {
  caseId: number;
  copy: AppealCopy;
}) {
  const { dict } = useI18n();
  const router = useRouter();
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const text = body.trim();
    if (Array.from(text).length < 10) {
      setErr(copy.tooShort);
      return;
    }
    setBusy(true);
    setErr(null);
    try {
      await api.post("/api/v1/cheat-appeals", {
        case_id: caseId,
        body: text,
      });
      setDone(true);
      setBody("");
      router.refresh();
    } catch (e2) {
      setErr(
        e2 instanceof ApiError
          ? apiErrorMessage(dict, e2)
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  if (done) {
    return (
      <p role="status" className="text-sm text-[var(--tone-success)]">
        {copy.ok}
      </p>
    );
  }

  return (
    <form onSubmit={submit} className="flex flex-col gap-2">
      <label className="flex flex-col gap-1">
        <span className="text-sm text-sub">{copy.label}</span>
        <textarea
          value={body}
          onChange={(e) => setBody(e.target.value)}
          required
          minLength={10}
          maxLength={4000}
          rows={4}
          placeholder={copy.placeholder}
          className="min-h-[88px] rounded-[var(--r-sm)] border border-line
            bg-[var(--cloud)] px-3 py-2 text-sm outline-none
            focus:border-[var(--accent)]"
        />
      </label>
      {err && (
        <p role="alert" className="text-sm text-danger">
          {err}
        </p>
      )}
      <button type="submit" disabled={busy} className={BTN_LG_SKY}>
        {busy ? copy.busy : copy.submit}
      </button>
    </form>
  );
}

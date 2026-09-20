"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 求种区发布表单（从 request-board.tsx 按域拆出）：
 *  添加求种（POST /requests）的标题/简介/悬赏三字段表单，
 *  提交成功后回调 onCreated 供父层刷新列表，onCancel 关闭表单。 */

export function RequestForm({
  onCreated,
  onCancel,
}: {
  onCreated: () => void;
  onCancel: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.requests;
  const [ftTitle, setFtTitle] = useState("");
  const [ftDescr, setFtDescr] = useState("");
  const [ftBounty, setFtBounty] = useState("100");
  const [ftBusy, setFtBusy] = useState(false);
  const [ftMsg, setFtMsg] = useState<string | null>(null);

  return (
    <form
      className="baozi-panel flex flex-col gap-2 p-4"
      onSubmit={async (e) => {
        e.preventDefault();
        const bounty = Number(ftBounty);
        if (!ftTitle.trim() || !Number.isFinite(bounty) || bounty < 0) {
          setFtMsg(t.formInvalid);
          return;
        }
        setFtBusy(true);
        setFtMsg(null);
        try {
          await api.post("/api/v1/requests", {
            title: ftTitle.trim(),
            descr: ftDescr.trim() || undefined,
            bounty,
          });
          setFtMsg(t.formOk.replace("{n}", String(bounty)));
          setFtTitle("");
          setFtDescr("");
          setFtBounty("100");
          onCreated();
        } catch (err) {
          setFtMsg(err instanceof Error ? err.message : t.formInvalid);
        } finally {
          setFtBusy(false);
        }
      }}
    >
      <label className="flex flex-col gap-1 text-sm">
        {t.formTitle}
        <input
          value={ftTitle}
          onChange={(e) => setFtTitle(e.target.value)}
          maxLength={200}
          required
          className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-3"
        />
      </label>
      <label className="flex flex-col gap-1 text-sm">
        {t.formDescr}
        <textarea
          value={ftDescr}
          onChange={(e) => setFtDescr(e.target.value)}
          maxLength={2000}
          rows={3}
          className="rounded-[var(--r-sm)] border border-line bg-cloud px-3 py-2"
        />
      </label>
      <label className="flex flex-col gap-1 text-sm">
        {t.formBounty}
        <input
          type="number"
          min={0}
          value={ftBounty}
          onChange={(e) => setFtBounty(e.target.value)}
          className="min-h-[40px] w-40 rounded-[var(--r-sm)] border border-line bg-cloud px-3"
        />
      </label>
      <div className="flex items-center gap-2">
        <button
          type="submit"
          disabled={ftBusy}
          className="min-h-[40px] rounded-full bg-sky px-5 text-sm font-bold text-white disabled:opacity-50"
        >
          {ftBusy ? t.formBusy : t.formSubmit}
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="min-h-[40px] rounded-full border border-line px-4 text-sm"
        >
          {t.formCancel}
        </button>
        {ftMsg && <span className="text-xs text-sub">{ftMsg}</span>}
      </div>
    </form>
  );
}

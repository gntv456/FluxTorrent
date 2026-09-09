"use client";

import { useRef, useState } from "react";
import { ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

export function UploadForm() {
  const { dict } = useI18n();
  // 字典分类/媒介数组按下标对齐：index 0 = 全部，1..n = 对应 id
  const categories = dict.torrents.categories.slice(1);
  const media = dict.torrents.media.slice(1);
  // grades 字典下标 i 与 grades 表 id（i-1）对齐；0 = 不选择
  const grades = dict.torrents.grades.slice(1);
  const editions = dict.upload.editions;
  const fileRef = useRef<HTMLInputElement>(null);
  const [categoryId, setCategoryId] = useState(2);
  const [mediumId, setMediumId] = useState(1);
  const [gradeId, setGradeId] = useState("");
  const [editionId, setEditionId] = useState("");
  const [smallDescr, setSmallDescr] = useState("");
  const [anonymous, setAnonymous] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const file = fileRef.current?.files?.[0];
    if (!file) {
      setMsg(dict.upload.chooseFile);
      return;
    }
    setBusy(true);
    setMsg(null);
    try {
      const form = new FormData();
      form.append("file", file);
      const qs = new URLSearchParams({
        category_id: String(categoryId),
        medium_id: String(mediumId),
        anonymous: String(anonymous),
      });
      if (gradeId) qs.set("grade_id", gradeId);
      if (editionId) qs.set("edition_id", editionId);
      if (smallDescr.trim()) qs.set("small_descr", smallDescr.trim());
      const base =
        process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";
      const res = await fetch(`${base}/api/v1/torrents?${qs}`, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${localStorage.getItem("flux.token") ?? ""}`,
        },
        body: form,
      });
      const body = await res.json();
      if (body.code !== 0) {
        setMsg(dict.errors[body.code] ?? body.message ?? dict.upload.fail);
      } else {
        setMsg(fmt(dict.upload.success, { id: body.data.id }));
        if (fileRef.current) fileRef.current.value = "";
      }
    } catch {
      setMsg(dict.upload.networkError);
    } finally {
      setBusy(false);
    }
  }

  const selectCls =
    "min-h-[44px] rounded-[var(--r-sm)] border border-line px-3 bg-white";

  return (
    <form
      onSubmit={submit}
      className="flex flex-col gap-3 rounded-[var(--r-lg)] border border-line bg-white p-5 shadow-[var(--shadow-card)]"
    >
      <label className="flex flex-col gap-1">
        <span className="text-sm text-sub">{dict.upload.fileLabel}</span>
        <input
          ref={fileRef}
          type="file"
          accept=".torrent,application/x-bittorrent"
          required
          className="min-h-[44px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 py-2"
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-sm text-sub">{dict.upload.smallDescr}</span>
        <input
          type="text"
          value={smallDescr}
          onChange={(e) => setSmallDescr(e.target.value)}
          placeholder={dict.upload.smallDescrPlaceholder}
          maxLength={120}
          className="min-h-[44px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
        />
      </label>
      <div className="grid grid-cols-2 gap-3">
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.upload.category}</span>
          <select
            value={categoryId}
            onChange={(e) => setCategoryId(Number(e.target.value))}
            className={selectCls}
          >
            {categories.map((label, i) => (
              <option key={i + 1} value={i + 1}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.upload.medium}</span>
          <select
            value={mediumId}
            onChange={(e) => setMediumId(Number(e.target.value))}
            className={selectCls}
          >
            {media.map((label, i) => (
              <option key={i + 1} value={i + 1}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.upload.grade}</span>
          <select
            value={gradeId}
            onChange={(e) => setGradeId(e.target.value)}
            className={selectCls}
          >
            <option value="">{dict.upload.gradeNone}</option>
            {grades.map((label, i) => (
              <option key={i} value={i}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.upload.edition}</span>
          <select
            value={editionId}
            onChange={(e) => setEditionId(e.target.value)}
            className={selectCls}
          >
            <option value="">{dict.upload.editionNone}</option>
            {editions.map((label, i) => (
              <option key={i + 1} value={i + 1}>
                {label}
              </option>
            ))}
          </select>
        </label>
      </div>
      <label className="flex items-center gap-2">
        <input
          type="checkbox"
          checked={anonymous}
          onChange={(e) => setAnonymous(e.target.checked)}
          className="h-5 w-5"
        />
        <span className="text-sm">{dict.upload.anonymous}</span>
      </label>
      <button
        type="submit"
        disabled={busy}
        className="min-h-[44px] rounded-full bg-coral font-bold text-white active:scale-[0.97] disabled:opacity-50"
      >
        {busy ? dict.upload.busy : dict.upload.submit}
      </button>
      {msg && <p role="status" className="text-sm text-ink">{msg}</p>}
    </form>
  );
}

"use client";

import { useRef, useState } from "react";
import { ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

/** 发布表单（NexusPHP 经典 rowhead/rowfollow 表格布局） */
export function UploadForm() {
  const { dict } = useI18n();
  // 字典分类/媒介数组按下标对齐：index 0 = 全部，1..n = 对应 id
  const categories = dict.torrents.categories.slice(1);
  const media = dict.torrents.media.slice(1);
  // grades 字典下标 i 与 grades 表 id（i-1）对齐；0 = 不选择
  const grades = dict.torrents.grades.slice(1);
  const editions = dict.upload.editions;
  const fileRef = useRef<HTMLInputElement>(null);
  const [fileName, setFileName] = useState("");
  const [categoryId, setCategoryId] = useState(2);
  const [mediumId, setMediumId] = useState(1);
  const [gradeId, setGradeId] = useState("");
  const [editionId, setEditionId] = useState("");
  const [smallDescr, setSmallDescr] = useState("");
  const [descr, setDescr] = useState("");
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
      if (descr.trim()) qs.set("descr", descr.trim());
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
        setFileName("");
        setDescr("");
      }
    } catch {
      setMsg(dict.upload.networkError);
    } finally {
      setBusy(false);
    }
  }

  const fieldCls =
    "min-h-[38px] w-full rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 text-sm text-ink outline-none focus:border-[var(--baozi-orange)]";

  const row = (label: string, content: React.ReactNode, key?: string) => (
    <tr key={key ?? label}>
      <td className="rowhead">{label}</td>
      <td className="rowfollow">{content}</td>
    </tr>
  );

  return (
    <form onSubmit={submit}>
      <table className="nexus-table nexus-form">
        <thead>
          <tr>
            <td className="colhead" colSpan={2}>
              {dict.upload.title}
            </td>
          </tr>
        </thead>
        <tbody>
          {row(
            dict.upload.fileLabel,
            <div className="flex flex-col gap-1">
              <label className="flex min-h-[64px] cursor-pointer items-center justify-center rounded-[var(--r-sm)] border border-dashed border-[var(--baozi-line)] bg-[#fff8eb] px-3 text-sm text-[#5f5143] hover:border-[var(--baozi-orange)]">
                <input
                  ref={fileRef}
                  type="file"
                  accept=".torrent,application/x-bittorrent"
                  required
                  className="sr-only"
                  onChange={(e) => setFileName(e.target.files?.[0]?.name ?? "")}
                />
                {fileName
                  ? `📎 ${fileName}`
                  : dict.upload.formFileHint}
              </label>
            </div>,
          )}
          {row(
            dict.upload.smallDescr,
            <input
              type="text"
              value={smallDescr}
              onChange={(e) => setSmallDescr(e.target.value)}
              placeholder={dict.upload.smallDescrPlaceholder}
              maxLength={120}
              className={fieldCls}
            />,
          )}
          {row(
            dict.upload.descr,
            <div className="flex flex-col gap-1">
              <textarea
                value={descr}
                onChange={(e) => setDescr(e.target.value)}
                placeholder={dict.upload.descrPlaceholder}
                rows={7}
                maxLength={10000}
                className={fieldCls}
              />
              <span className="text-xs text-sub">{dict.upload.descrHint}</span>
            </div>,
          )}
          {row(
            dict.upload.category,
            <select
              value={categoryId}
              onChange={(e) => setCategoryId(Number(e.target.value))}
              className={fieldCls}
            >
              {categories.map((label, i) => (
                <option key={i + 1} value={i + 1}>
                  {label}
                </option>
              ))}
            </select>,
          )}
          {row(
            dict.upload.medium,
            <select
              value={mediumId}
              onChange={(e) => setMediumId(Number(e.target.value))}
              className={fieldCls}
            >
              {media.map((label, i) => (
                <option key={i + 1} value={i + 1}>
                  {label}
                </option>
              ))}
            </select>,
          )}
          {row(
            dict.upload.grade,
            <select
              value={gradeId}
              onChange={(e) => setGradeId(e.target.value)}
              className={fieldCls}
            >
              <option value="">{dict.upload.gradeNone}</option>
              {grades.map((label, i) => (
                <option key={i} value={i}>
                  {label}
                </option>
              ))}
            </select>,
          )}
          {row(
            dict.upload.edition,
            <select
              value={editionId}
              onChange={(e) => setEditionId(e.target.value)}
              className={fieldCls}
            >
              <option value="">{dict.upload.editionNone}</option>
              {editions.map((label, i) => (
                <option key={i + 1} value={i + 1}>
                  {label}
                </option>
              ))}
            </select>,
          )}
          {row(
            dict.upload.anonymous,
            <label className="flex cursor-pointer items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={anonymous}
                onChange={(e) => setAnonymous(e.target.checked)}
                className="h-4 w-4 accent-[var(--baozi-orange)]"
              />
              {dict.upload.anonymous}
            </label>,
          )}
          <tr>
            <td className="rowfollow" colSpan={2}>
              <div className="flex flex-wrap items-center gap-3">
                <button
                  type="submit"
                  disabled={busy}
                  className="min-h-[38px] rounded-[10px] bg-[linear-gradient(135deg,var(--baozi-orange-bright),var(--baozi-orange))] px-5 text-sm font-bold text-white shadow-[var(--shadow-card)] hover:bg-[linear-gradient(135deg,var(--baozi-orange),var(--baozi-orange-dark))] active:scale-[0.97] disabled:opacity-50"
                >
                  {busy ? dict.upload.busy : dict.upload.submit}
                </button>
                {msg && (
                  <p role="status" className="text-sm text-ink">
                    {msg}
                  </p>
                )}
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </form>
  );
}

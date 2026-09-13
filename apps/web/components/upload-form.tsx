"use client";

import { useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

interface ProfileCat { id: number; name: string }
interface SectionDictRow { id: number; kind: string; name: string; sort: number; mode_id: number | null }

/** 第八轮 Section 多维：发布表单新维度（kind → 中文名） */
const SECTION_KINDS: [string, string][] = [
  ["codec", "编码"],
  ["audio_codec", "音频编码"],
  ["standard", "规格"],
  ["team", "制作组"],
  ["source", "来源"],
  ["processing", "处理工艺"],
];

/** 发布表单（NexusPHP 经典 rowhead/rowfollow 表格布局；分类来自站点档案，支持任意类型 PT 站） */
export function UploadForm() {
  const { dict } = useI18n();
  // 分类以 /site-profile 为准（类型包可切换）；字典仅兜底
  const [profileCats, setProfileCats] = useState<ProfileCat[] | null>(null);
  useEffect(() => {
    api.get<{ categories: ProfileCat[] }>("/api/v1/site-profile")
      .then((p) => setProfileCats(p.categories))
      .catch(() => setProfileCats([]));
  }, []);
  const categories = profileCats
    ? profileCats.map((c) => c.name)
    : dict.torrents.categories.slice(1);
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
  const [poster, setPoster] = useState("");
  const [anonymous, setAnonymous] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // 第八轮 Section 多维：字典与所选值（kind → dict_id，可留空）
  const [secDict, setSecDict] = useState<Record<string, SectionDictRow[]>>({});
  const [secVals, setSecVals] = useState<Record<string, string>>({});
  useEffect(() => {
    api.get<Record<string, SectionDictRow[]>>("/api/v1/section-dict")
      .then((d) => setSecDict(d))
      .catch(() => setSecDict({}));
  }, []);

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
      if (poster.trim()) qs.set("poster", poster.trim());
      // 第八轮 Section 多维：非空维度打包成 sections JSON
      const sections = Object.fromEntries(
        Object.entries(secVals).filter(([, v]) => v),
      );
      if (Object.keys(sections).length > 0) qs.set("sections", JSON.stringify(sections));
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
              <label className="flex min-h-[64px] cursor-pointer items-center justify-center rounded-[var(--r-sm)] border border-dashed border-[var(--baozi-line)] bg-[var(--head-b)] px-3 text-sm text-[var(--text-body)] hover:border-[var(--baozi-orange)]">
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
            dict.upload.poster ?? "封面图 URL",
            <div className="flex flex-col gap-1">
              <input
                type="url"
                value={poster}
                onChange={(e) => setPoster(e.target.value)}
                placeholder="https://…（种子列表封面位与首页海报墙共用，留空则显示分类色块）"
                maxLength={500}
                className={fieldCls}
              />
              <span className="text-xs text-sub">
                {dict.upload.posterHint ?? "外链图床 URL；建议竖版海报比例"}
              </span>
            </div>,
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
              {(profileCats ?? categories.map((name, i) => ({ id: i + 1, name }))).map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
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
          {SECTION_KINDS.map(([kind, label]) => (
            (secDict[kind]?.length ?? 0) > 0 &&
            row(
              label,
              <select
                value={secVals[kind] ?? ""}
                onChange={(e) => setSecVals((prev) => ({ ...prev, [kind]: e.target.value }))}
                className={fieldCls}
              >
                <option value="">（不选择）</option>
                {secDict[kind].map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.name}
                  </option>
                ))}
              </select>,
              kind,
            )
          ))}
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

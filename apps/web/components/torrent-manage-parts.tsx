"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { normTagRow, type TagPayload } from "@/components/torrent-tags";

type Dict = ReturnType<typeof useI18n>["dict"];

/** 编辑弹层·标签/推荐位/基础字段/元数据块（0184 从 torrent-manage.tsx
 *  按域拆出，300 行门禁）：标签按 attribute/content 分组（与发布页
 *  upload-form-quality 同口径）；推荐位（NP 挑选）staff 专属。 */

export const FLD =
  "min-h-[36px] rounded-[var(--r-sm)] border border-line px-2 text-sm";

/** 操作按钮（pill 形态）：编辑/补种/删除/保存/取消行共用 */

export const ATTACH_ACCEPT = [
  "image/png",
  "image/jpeg",
  "image/gif",
  "image/webp",
  "image/avif",
  "application/pdf",
  "text/plain",
].join(",");

export const PILL =
  "min-h-[36px] rounded-full px-4 text-xs font-bold disabled:opacity-50";

/** 标签整组编辑（0159 P1 + 0184 分组）：普通标签 checkbox；
 *  official 类不出现（详情页 toggle 走 staff 口径） */
export function ManageTagsBlock({
  tagDict,
  fTags,
  setFTags,
}: {
  tagDict?: TagPayload["dict"];
  fTags: number[];
  setFTags: React.Dispatch<React.SetStateAction<number[]>>;
}) {
  const { dict } = useI18n();
  const dictRows = (tagDict ?? []).map(normTagRow);
  if (!dictRows.some((d) => d.kind !== "official")) return null;
  return (
    <fieldset className="flex flex-col gap-1 text-xs">
      <legend>{dict.torrTags2.title}</legend>
      {(["attribute", "content"] as const)
        .filter((g) =>
          dictRows.some(
            (d) => d.kind !== "official" && (d.group ?? "attribute") === g,
          ),
        )
        .map((g) => (
          <div key={g} className="flex flex-wrap items-center gap-1.5">
            {dictRows.some((d) => d.group === "content") && (
              <span className="text-[11px] font-bold text-sub">
                {g === "attribute"
                  ? dict.upload.tagGroupAttr
                  : dict.upload.tagGroupContent}
                ：
              </span>
            )}
            {dictRows
              .filter(
                (d) =>
                  d.kind !== "official" && (d.group ?? "attribute") === g,
              )
              .map((d) => {
                const on = fTags.includes(d.id);
                return (
                  <label
                    key={d.id}
                    className={`td-tag torrents-tag--user ${
                    on ? "" : "td-tag--off"
                  }`}
                    style={{
                      background: on ? d.bg_color || "var(--sky)" : undefined,
                      color: on ? d.color || "#fff" : undefined,
                      cursor: "pointer",
                    }}
                  >
                    <input
                      type="checkbox"
                      className="sr-only"
                      checked={on}
                      onChange={(e) =>
                        setFTags((prev) =>
                          e.target.checked
                            ? [...prev, d.id]
                            : prev.filter((x) => x !== d.id),
                        )
                      }
                    />
                    {d.name}
                  </label>
                );
              })}
          </div>
        ))}
    </fieldset>
  );
}

/** 推荐位（0184 对齐发布页，NP 挑选口径，staff 专属）：
 *  置顶位置/截止 + 推荐影片。datetime-local 值为本地格式 YYYY-MM-DDTHH:mm */
export function ManagePickBlock({
  fPos,
  setFPos,
  fPosUntil,
  setFPosUntil,
  fPick,
  setFPick,
}: {
  fPos: number;
  setFPos: (v: number) => void;
  fPosUntil: string;
  setFPosUntil: (v: string) => void;
  fPick: number;
  setFPick: (v: number) => void;
}) {
  const { dict } = useI18n();
  return (
    <fieldset className="flex flex-col gap-1 text-xs">
      <legend>{dict.upload.recommend}</legend>
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
        <label className="flex items-center gap-1">
          <span className="whitespace-nowrap text-sub">
            {dict.upload.pickPos}：
          </span>
          <select
            value={fPos}
            onChange={(e) => setFPos(Number(e.target.value))}
            className={FLD}
          >
            <option value={0}>{dict.upload.pickNone}</option>
            <option value={1}>{dict.upload.pickL1}</option>
            <option value={2}>{dict.upload.pickL2}</option>
          </select>
        </label>
        {fPos > 0 && (
          <label className="flex items-center gap-1">
            <span className="whitespace-nowrap text-sub">
              {dict.upload.pickUntil}：
            </span>
            <input
              type="datetime-local"
              value={fPosUntil}
              onChange={(e) => setFPosUntil(e.target.value)}
              className={FLD}
            />
          </label>
        )}
        <label className="flex items-center gap-1">
          <span className="whitespace-nowrap text-sub">
            {dict.upload.recommendMovie}：
          </span>
          <select
            value={fPick}
            onChange={(e) => setFPick(Number(e.target.value))}
            className={FLD}
          >
            <option value={0}>{dict.upload.recommendNone}</option>
            <option value={1}>{dict.upload.recommendNormal}</option>
            <option value={2}>{dict.upload.recommendClassic}</option>
          </select>
        </label>
      </div>
      <span className="text-[11px] text-sub">{dict.upload.recommendHint}</span>
    </fieldset>
  );
}

/** 图床附件（0184 对齐发布页 upload-form-files）：POST /attachments，
 *  成功后 [img]URL[/img] 追加进简介；凭证走 HttpOnly cookie */
export function useAttachmentUpload(
  setFDescr: React.Dispatch<React.SetStateAction<string>>,
) {
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  async function upload(file: File) {
    setBusy(true);
    setMsg(null);
    try {
      const fd = new FormData();
      fd.append("file", file);
      const res = await fetch("/api/v1/attachments", {
        method: "POST",
        body: fd,
      });
      const j = (await res.json()) as {
        data?: { url?: string; deduplicated?: boolean };
        message?: string;
      };
      if (!res.ok || !j.data?.url) throw new Error(j.message ?? "上传失败");
      const tag = file.type.startsWith("image/")
        ? `[img]${j.data.url}[/img]`
        : `[url=${j.data.url}]${file.name}[/url]`;
      setFDescr((prev) => (prev.trim() ? `${prev.trim()}\n\n` : "") + tag);
      setMsg(
        j.data.deduplicated
          ? "秒传成功（服务器已有同文件）"
          : "上传成功，已插入简介",
      );
    } catch (e) {
      setMsg(e instanceof Error ? e.message : "上传失败");
    } finally {
      setBusy(false);
    }
  }
  return { busy, msg, upload };
}


/** 多维质量编辑状态 + 分类联动（0184 从 torrent-manage.tsx 拆出）：
 *  换分类按 mode_id 重取 section-dict（归属模式决定该分类能填哪些维度），
 *  被模式隐藏/换批后不存在的字典项残留取值会被清洗（不再提交） */
export function useEditSections(
  sections: Record<string, number>,
  secKinds: { kind: string; label: string }[],
  secDict: Record<string, { id: number; name: string }[]>,
  categoryId: number,
) {
  const [fCat, setFCat] = useState(categoryId);
  // 多维质量（0087）：kind → dict_id；空串 = 清空该维
  const [fSec, setFSec] = useState<Record<string, number>>(() => {
    const o: Record<string, number> = {};
    for (const k of secKinds) o[k.kind] = sections[k.kind] ?? 0;
    return o;
  });
  // 分类联动维度（0184）：换分类按 mode_id 重取 section-dict——
  // 归属模式决定「这个分类能填哪些维度」，与发布页 useEffect 同口径
  const [catKinds, setCatKinds] = useState(secKinds);
  const [catDict, setCatDict] = useState(secDict);
  useEffect(() => {
    if (fCat === categoryId) return; // 初始分类直接用服务端传入的字典
    let alive = true;
    api
      .get<
        Record<string, unknown> & { kinds?: { kind: string; label: string }[] }
      >(
        `/api/v1/section-dict?category_id=${fCat}`,
      )
      .then((d) => {
        if (!alive) return;
        const kinds = d.kinds ?? [];
        const rest: Record<string, { id: number; name: string }[]> = {};
        for (const [k, v] of Object.entries(d)) {
          if (k !== "kinds" && k !== "modes")
            rest[k] = v as { id: number; name: string }[];
        }
        setCatKinds(kinds);
        setCatDict(rest);
        // 被模式隐藏/换批后不存在的字典项：残留取值不能再被提交
        setFSec((prev) => {
          const kept: Record<string, number> = {};
          for (const k of kinds) {
            const v = prev[k.kind] ?? 0;
            if (v > 0 && (rest[k.kind] ?? []).some((r) => r.id === v)) {
              kept[k.kind] = v;
            }
          }
          return kept;
        });
      })
      .catch(() => setCatKinds([]));
    return () => {
      alive = false;
    };
  }, [fCat, categoryId]);
  return { fCat, setFCat, fSec, setFSec, catKinds, catDict };
}

"use client";

import { useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { EDITIONS } from "@/lib/format";
import type { TagPayload } from "@/components/torrent-tags";
import { normTagRow } from "@/components/torrent-tags";
import { Modal } from "@/components/modal";
import { UploadDescrBlock } from "@/components/upload-form-descr";

/** 种子作者/管理操作（NP edit.php/delete.php 口径）：
 *  编辑（全字段：名称/副题/分类/媒介/学段/版本/多维质量/封面/简介/PT-Gen/
 *  MediaInfo/IMDB/匿名/标签/定价）→ 普通作者回退待审（staff/免审等级不回退）；
 *  删除（软删）。0173：字段对齐发布页（UploadForm），弹层改覆盖式 Modal。 */

type Dict = ReturnType<typeof useI18n>["dict"];

export function TorrentManage({
  torrentId,
  name,
  smallDescr,
  descr,
  anonymous,
  categoryId,
  mediumId,
  gradeId,
  editionId,
  price,
  posterUrl,
  mediainfo,
  sections,
  secKinds,
  secDict,
  cats,
  seeders,
  imdbId,
  tagDict,
  tagMine,
  autoOpen,
}: {
  torrentId: number;
  name: string;
  smallDescr: string | null;
  descr: string | null;
  anonymous: boolean;
  categoryId: number;
  /** 媒介/学段/版本（0173 对齐发布页）：老数据列，null = 未设 */
  mediumId: number | null;
  gradeId: number | null;
  editionId: number | null;
  /** 付费价格（0086 独立端点 PUT /price） */
  price: number;
  /** 封面外链（media_info.poster） */
  posterUrl: string | null;
  /** MediaInfo 全文（media_info.mediainfo） */
  mediainfo: string | null;
  /** 当前多维质量值（0087）：kind → dict_id */
  sections: Record<string, number>;
  secKinds: { kind: string; label: string }[];
  secDict: Record<string, { id: number; name: string }[]>;
  cats: { id: number; name: string }[];
  seeders?: number;
  imdbId?: string | null;
  /** 标签字典与已选（0159 P1：详情页 aggregate 已带回，编辑表单免二次请求） */
  tagDict?: TagPayload["dict"];
  tagMine?: number[];
  /** 深链（列表编辑按钮 /torrent/{id}?edit=1）自动展开 */
  autoOpen?: boolean;
}) {
  const { dict } = useI18n();
  const router = useRouter();
  const t = dict.torrentManage2;
  const [open, setOpen] = useState(Boolean(autoOpen));
  const [fName, setFName] = useState(name);
  const [fSub, setFSub] = useState(smallDescr ?? "");
  const [fDescr, setFDescr] = useState(descr ?? "");
  const [fAnon, setFAnon] = useState(anonymous);
  const [fImdb, setFImdb] = useState(imdbId ?? "");
  const [fCat, setFCat] = useState(categoryId);
  // 0173 对齐发布页：媒介/学段/版本（下拉 index+1 = id，0 = 未设不提交）、
  // 封面外链、MediaInfo、付费价格（独立端点）、PT-Gen
  const [fMedium, setFMedium] = useState(mediumId ?? 0);
  const [fGrade, setFGrade] = useState(gradeId ?? 0);
  const [fEdition, setFEdition] = useState(editionId ?? 0);
  const [fPrice, setFPrice] = useState(price);
  const [fPoster, setFPoster] = useState(posterUrl ?? "");
  const [fMediainfo, setFMediainfo] = useState(mediainfo ?? "");
  const [fPtgenUrl, setFPtgenUrl] = useState("");
  const [ptgenBusy, setPtgenBusy] = useState(false);
  // 多维质量（0087）：kind → dict_id；空串 = 清空该维
  const [fSec, setFSec] = useState<Record<string, number>>(() => {
    const o: Record<string, number> = {};
    for (const k of secKinds) o[k.kind] = sections[k.kind] ?? 0;
    return o;
  });
  // 标签整组编辑（0159 P1）：与发布表单同交互——普通标签 checkbox；
  // official 类不出现（详情页 toggle 走 staff 口径，这里不重复实现权限分支）
  const dictRows = (tagDict ?? []).map(normTagRow);
  const [fTags, setFTags] = useState<number[]>(() =>
    (tagMine ?? []).filter((id) =>
      dictRows.some((d) => d.id === id && d.kind !== "official"),
    ),
  );
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const descrRef = useRef<HTMLTextAreaElement>(null);

  // 深链 ?edit=1 只消费一次：关闭弹层后清参数，刷新/分享不带出
  useEffect(() => {
    if (!autoOpen) return;
    const url = new URL(window.location.href);
    url.searchParams.delete("edit");
    window.history.replaceState(null, "", url);
  }, [autoOpen]);

  /** PT-Gen（0173 对齐发布页）：服务端代理拉条目信息，descr 追加进简介 */
  async function gen() {
    const u = fPtgenUrl.trim();
    if (!u) return;
    setPtgenBusy(true);
    setMsg(null);
    try {
      const r = await api.get<{ name: string; descr: string }>(
        `/api/v1/ptgen?url=${encodeURIComponent(u)}`,
      );
      setFDescr((prev) => (prev ? `${prev}\n\n${r.descr}` : r.descr));
      if (r.name && !fName.trim()) setFName(r.name);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setPtgenBusy(false);
    }
  }

  async function save() {
    setBusy(true);
    setMsg(null);
    try {
      await api.put(`/api/v1/torrents/${torrentId}`, {
        name: fName.trim(),
        small_descr: fSub.trim(),
        descr: fDescr,
        anonymous: fAnon,
        category_id: fCat,
        imdb_id: fImdb.trim() || "",
        // 0173 补齐发布页字段：媒介/学段/版本（0 = 未设不提交）、封面外链、
        // MediaInfo（None=不动 / Some("")=清除 / Some(text)=写入——始终提交）
        ...(fMedium > 0 ? { medium_id: fMedium } : {}),
        ...(fGrade > 0 ? { grade_id: fGrade } : {}),
        ...(fEdition > 0 ? { edition_id: fEdition } : {}),
        poster: fPoster.trim(),
        mediainfo: fMediainfo.trim(),
        // 多维质量：有值的维以 {kind: dict_id} 提交（后端写 torrent_sections）
        sections: Object.fromEntries(
          Object.entries(fSec).filter(([, v]) => v > 0),
        ),
        // 标签整组提交（official 类保留不动：前端只编辑普通标签，
        // 后端 DELETE+apply 会把 official 一并清掉，所以这里带上原 official 集）
        tag_ids: [
          ...fTags,
          ...(tagMine ?? []).filter((id) =>
            dictRows.some((d) => d.id === id && d.kind === "official"),
          ),
        ],
      });
      // 价格（0086 独立端点）：仅变化时调用（0 = 恢复免费）
      if (fPrice !== price) {
        await api.put(`/api/v1/torrents/${torrentId}/price`, {
          price: Math.min(1_000_000, Math.max(0, fPrice)),
        });
      }
      setMsg(t.saved);
      setOpen(false);
      router.refresh();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function reseed() {
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ notified: number }>(
        `/api/v1/torrents/${torrentId}/reseed`,
        {},
      );
      setMsg(dict.reseed2.ok.replace("{n}", String(r.notified)));
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function del() {
    if (!window.confirm(t.delConfirm)) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.del(`/api/v1/torrents/${torrentId}`);
      setMsg(t.deleted);
      router.push("/torrents");
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  const fld =
    "min-h-[36px] rounded-[var(--r-sm)] border border-line px-2 text-sm";
  // 0173：媒介/学段/版本下拉选项（数组 index+1 = id；0 位是列表筛选用占位）
  const mediaOpts = dict.torrents.media.slice(1);
  const gradeOpts = dict.torrents.grades.slice(1);
  const editionOpts = EDITIONS.slice(1);

  return (
    <div className="flex flex-col items-start gap-2">
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-sky-deep"
        >
          ✎ {t.edit}
        </button>
        {(seeders ?? 1) === 0 && (
          <button
            type="button"
            disabled={busy}
            onClick={reseed}
            className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-sun disabled:opacity-50"
            title={dict.reseed2.note}
          >
            🔄 {dict.reseed2.btn}
          </button>
        )}
        <button
          type="button"
          disabled={busy}
          onClick={del}
          className="min-h-[36px] rounded-full border border-tomato/60 px-4 text-xs font-bold text-tomato disabled:opacity-50"
        >
          🗑 {t.del}
        </button>
      </div>

      {/* 0173：编辑改覆盖式弹窗——不再内嵌展开挤动详情页布局 */}
      <Modal
        open={open}
        onClose={() => setOpen(false)}
        title={`✎ ${t.edit}`}
        size="lg"
      >
        <div className="flex w-full flex-col gap-2">
          <label className="flex flex-col gap-1 text-xs">
            {t.fieldName}
            <input
              value={fName}
              onChange={(e) => setFName(e.target.value)}
              className={fld}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            {t.fieldSub}
            <input
              value={fSub}
              onChange={(e) => setFSub(e.target.value)}
              className={fld}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            {dict.upload.category}
            <select
              value={fCat}
              onChange={(e) => setFCat(Number(e.target.value))}
              className={fld}
            >
              {cats.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
          </label>
          {/* 0173 对齐发布页：媒介/学段/版本（老数据列，api TorrentEditReq 直接支持） */}
          <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-xs">
            <label className="flex items-center gap-1">
              <span className="whitespace-nowrap text-sub">
                {dict.torrent.medium}：
              </span>
              <select
                value={fMedium}
                onChange={(e) => setFMedium(Number(e.target.value))}
                className={fld}
              >
                <option value={0}>{dict.upload.gradeNone}</option>
                {mediaOpts.map((name, i) => (
                  <option key={i + 1} value={i + 1}>
                    {name}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex items-center gap-1">
              <span className="whitespace-nowrap text-sub">
                {dict.torrent.grade}：
              </span>
              <select
                value={fGrade}
                onChange={(e) => setFGrade(Number(e.target.value))}
                className={fld}
              >
                <option value={0}>{dict.upload.gradeNone}</option>
                {gradeOpts.map((name, i) => (
                  <option key={i + 1} value={i + 1}>
                    {name}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex items-center gap-1">
              <span className="whitespace-nowrap text-sub">
                {dict.torrent.edition}：
              </span>
              <select
                value={fEdition}
                onChange={(e) => setFEdition(Number(e.target.value))}
                className={fld}
              >
                <option value={0}>{dict.upload.gradeNone}</option>
                {editionOpts.map((name, i) => (
                  <option key={i + 1} value={i + 1}>
                    {name}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {/* 多维质量（0087 同发布表单）：kind 下拉，空 = 不设 */}
          {secKinds.length > 0 && (
            <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-xs">
              {secKinds.map((k) => (
                <label key={k.kind} className="flex items-center gap-1">
                  <span className="whitespace-nowrap text-sub">
                    {k.label}：
                  </span>
                  <select
                    value={fSec[k.kind] ?? 0}
                    onChange={(e) =>
                      setFSec((prev) => ({
                        ...prev,
                        [k.kind]: Number(e.target.value),
                      }))
                    }
                    className={fld}
                  >
                    <option value={0}>{dict.upload.gradeNone}</option>
                    {(secDict[k.kind] ?? []).map((o) => (
                      <option key={o.id} value={o.id}>
                        {o.name}
                      </option>
                    ))}
                  </select>
                </label>
              ))}
            </div>
          )}
          {/* 0173 对齐发布页：封面外链 + 付费价格 */}
          <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-xs">
            <label className="flex items-center gap-1">
              <span className="whitespace-nowrap text-sub">
                {dict.upload.poster}：
              </span>
              <input
                value={fPoster}
                placeholder="https://…"
                onChange={(e) => setFPoster(e.target.value)}
                className={`${fld} w-56`}
              />
            </label>
            <label className="flex items-center gap-1">
              <span className="whitespace-nowrap text-sub">
                {dict.upload.price}：
              </span>
              <input
                type="number"
                min={0}
                max={1000000}
                value={fPrice}
                onChange={(e) => setFPrice(Number(e.target.value) || 0)}
                className={`${fld} w-28`}
              />
              <span className="max-w-40 text-[11px] text-sub">
                {dict.upload.priceHint}
              </span>
            </label>
          </div>
          {/* 0173 对齐发布页：PT-Gen 生成简介 + MediaInfo */}
          <div className="flex flex-wrap items-center gap-2 text-xs">
            <span className="whitespace-nowrap font-bold">
              {dict.upload.ptgen}
            </span>
            <input
              value={fPtgenUrl}
              placeholder={dict.upload.ptgenPlaceholder}
              onChange={(e) => setFPtgenUrl(e.target.value)}
              className={`${fld} w-64`}
            />
            <button
              type="button"
              disabled={ptgenBusy || !fPtgenUrl.trim()}
              onClick={() => void gen()}
              className="min-h-[36px] rounded-full bg-sky px-4 text-xs font-bold text-white disabled:opacity-50"
            >
              {ptgenBusy ? dict.upload.ptgenBusy : dict.upload.ptgenBtn}
            </button>
            <span className="text-[11px] text-sub">
              {dict.upload.ptgenHint}
            </span>
          </div>
          {/* 简介与 BBCode 工具条（0173 复用发布页组件） */}
          <UploadDescrBlock
            descr={fDescr}
            setDescr={setFDescr}
            descrRef={descrRef}
          />
          <label className="flex flex-col gap-1 text-xs">
            MediaInfo
            <textarea
              value={fMediainfo}
              rows={4}
              onChange={(e) => setFMediainfo(e.target.value)}
              className="rounded-[var(--r-sm)] border border-line px-2 py-1 font-mono text-xs"
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            {t.fieldImdb ?? "IMDB"}
            <input
              value={fImdb}
              placeholder="tt1234567"
              onChange={(e) =>
                setFImdb(e.target.value.replace(/[^tT0-9]/g, "").slice(0, 10))
              }
              className="w-40 rounded-[var(--r-sm)] border border-line px-2 py-1 text-sm"
            />
            <span className="text-[11px] text-sub">{t.fieldImdbNote}</span>
          </label>
          <label className="flex items-center gap-2 text-xs">
            <input
              type="checkbox"
              checked={fAnon}
              onChange={(e) => setFAnon(e.target.checked)}
            />
            {t.fieldAnonymous}
          </label>
          {/* 标签编辑（0159 P1）：普通标签 checkbox 整组提交 */}
          {dictRows.some((d) => d.kind !== "official") && (
            <fieldset className="flex flex-col gap-1 text-xs">
              <legend>{dict.torrTags2.title}</legend>
              <div className="flex flex-wrap gap-1.5">
                {dictRows
                  .filter((d) => d.kind !== "official")
                  .map((d) => (
                    <label
                      key={d.id}
                      className={`td-tag torrents-tag--user ${fTags.includes(d.id) ? "" : "td-tag--off"}`}
                      style={{
                        background: fTags.includes(d.id)
                          ? d.bg_color || "var(--sky)"
                          : undefined,
                        color: fTags.includes(d.id)
                          ? d.color || "#fff"
                          : undefined,
                        cursor: "pointer",
                      }}
                    >
                      <input
                        type="checkbox"
                        className="sr-only"
                        checked={fTags.includes(d.id)}
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
                  ))}
              </div>
            </fieldset>
          )}
          <div className="flex items-center gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={save}
              className="min-h-[36px] rounded-full bg-sky-deep px-5 text-xs font-bold text-white disabled:opacity-50"
            >
              {t.save}
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => setOpen(false)}
              className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-sub disabled:opacity-50"
            >
              {dict.common.cancel}
            </button>
          </div>
        </div>
      </Modal>

      {msg && (
        <p className="text-xs text-sub" role="status">
          {msg}
        </p>
      )}
    </div>
  );
}

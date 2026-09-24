"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { TagPayload } from "@/components/torrent-tags";
import { normTagRow } from "@/components/torrent-tags";

/** 种子作者/管理操作（NP edit.php/delete.php 口径）：
 *  编辑（全字段：名称/副题/分类/多维质量/简介/IMDB/匿名/标签整组）→ 回退待审；
 *  删除（软删，作者仅限未过审，staff 任意）。
 *  列表「编辑」深链 /torrent/{id}?edit=1 到达后自动展开弹层。 */

type Dict = ReturnType<typeof useI18n>["dict"];

export function TorrentManage({
  torrentId,
  name,
  smallDescr,
  descr,
  anonymous,
  categoryId,
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

  // 深链 ?edit=1 只消费一次：关闭弹层后清参数，刷新/分享不带出
  useEffect(() => {
    if (!autoOpen) return;
    const url = new URL(window.location.href);
    url.searchParams.delete("edit");
    window.history.replaceState(null, "", url);
  }, [autoOpen]);

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

      {open && (
        <div
          role="dialog"
          aria-label={t.edit}
          className="flex w-full flex-col gap-2 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]"
        >
          <h3 className="text-sm font-bold">{t.edit}</h3>
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
          <label className="flex flex-col gap-1 text-xs">
            {t.fieldDescr}
            <textarea
              value={fDescr}
              onChange={(e) => setFDescr(e.target.value)}
              rows={8}
              className="rounded-[var(--r-sm)] border border-line px-2 py-1 text-sm"
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
      )}

      {msg && (
        <p className="text-xs text-sub" role="status">
          {msg}
        </p>
      )}
    </div>
  );
}

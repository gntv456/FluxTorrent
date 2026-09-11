"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

/** 种子作者/管理操作（NP edit.php/delete.php 口径）：
 *  编辑（名称/副标题/简介/匿名）→ 回退待审；删除（软删，作者仅限未过审，staff 任意） */
export function TorrentManage({
  torrentId,
  name,
  smallDescr,
  descr,
  anonymous,
  seeders,
}: {
  torrentId: number;
  name: string;
  smallDescr: string | null;
  descr: string | null;
  anonymous: boolean;
  seeders?: number;
}) {
  const { dict } = useI18n();
  const router = useRouter();
  const t = dict.torrentManage2 ?? {
    edit: "编辑",
    editing: "编辑种子",
    fieldName: "名称",
    fieldSub: "副标题",
    fieldDescr: "简介",
    fieldAnonymous: "匿名发布",
    save: "保存（回退待审）",
    del: "删除",
    delConfirm: "确认删除该种子？（作者仅可删除未过审种子）",
    saved: "已保存，等待重新审核",
    deleted: "已删除",
  };
  const [open, setOpen] = useState(false);
  const [fName, setFName] = useState(name);
  const [fSub, setFSub] = useState(smallDescr ?? "");
  const [fDescr, setFDescr] = useState(descr ?? "");
  const [fAnon, setFAnon] = useState(anonymous);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function save() {
    setBusy(true);
    setMsg(null);
    try {
      await api.put(`/api/v1/torrents/${torrentId}`, {
        name: fName.trim(),
        small_descr: fSub.trim(),
        descr: fDescr,
        anonymous: fAnon,
      });
      setMsg(t.saved);
      setOpen(false);
      router.refresh();
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    } finally {
      setBusy(false);
    }
  }

  async function reseed() {
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ notified: number }>(`/api/v1/torrents/${torrentId}/reseed`, {});
      setMsg((dict.reseed2?.ok ?? "已通知 {n} 位下载者").replace("{n}", String(r.notified)));
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
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
      setMsg(apiErrorMessage(dict, e));
    } finally {
      setBusy(false);
    }
  }

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
            title={dict.reseed2?.note ?? "向所有完成下载的用户发送补种请求（15 分钟限频）"}
          >
            🔄 {dict.reseed2?.btn ?? "请求补种"}
          </button>
        )}
        <button
          type="button"
          disabled={busy}
          onClick={del}
          className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-danger disabled:opacity-50"
        >
          🗑 {t.del}
        </button>
      </div>

      {open && (
        <div className="flex w-full flex-col gap-2 rounded-[var(--r-md)] border border-line bg-white p-3">
          <label className="flex flex-col gap-1 text-xs">
            {t.fieldName}
            <input
              value={fName}
              onChange={(e) => setFName(e.target.value)}
              className="min-h-[36px] rounded-[var(--r-sm)] border border-line px-2 text-sm"
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            {t.fieldSub}
            <input
              value={fSub}
              onChange={(e) => setFSub(e.target.value)}
              className="min-h-[36px] rounded-[var(--r-sm)] border border-line px-2 text-sm"
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            {t.fieldDescr}
            <textarea
              value={fDescr}
              onChange={(e) => setFDescr(e.target.value)}
              rows={6}
              className="rounded-[var(--r-sm)] border border-line px-2 py-1 text-sm"
            />
          </label>
          <label className="flex items-center gap-2 text-xs">
            <input type="checkbox" checked={fAnon} onChange={(e) => setFAnon(e.target.checked)} />
            {t.fieldAnonymous}
          </label>
          <button
            type="button"
            disabled={busy}
            onClick={save}
            className="min-h-[36px] w-fit rounded-full bg-sky px-4 text-xs font-bold text-white disabled:opacity-50"
          >
            {t.save}
          </button>
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

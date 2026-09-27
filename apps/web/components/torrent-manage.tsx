"use client";

import { useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { normTagRow, type TagPayload } from "@/components/torrent-tags";
import type { SectionKindMeta } from "@/components/admin-sections-shared";
import { Modal } from "@/components/modal";
import { UploadDescrBlock } from "@/components/upload-form-descr";
import {
  FLD,
  PILL,
  ManagePickBlock,
  ManageTagsBlock,
  useAttachmentUpload,
  useEditSections,
} from "@/components/torrent-manage-parts";
import { ManageBasicFields, ManageMetaFields } from "@/components/torrent-manage-fields";
import {
  buildEditPayload,
  useTorrentActions,
} from "@/components/torrent-manage-submit";

/** 种子作者/管理操作（NP edit.php/delete.php 口径）：编辑全字段 →
 *  普通作者回退待审（staff/免审等级不回退）；删除（软删）。0173 对齐
 *  发布页；0184 补齐剩余差距；表单块拆至 -parts/-fields/-submit
 *  （300 行门禁），本文件保留状态与提交逻辑。 */

export interface TorrentManageProps {
  torrentId: number;
  name: string;
  smallDescr: string | null;
  descr: string | null;
  anonymous: boolean;
  categoryId: number;
  /** 付费价格（0086 独立端点 PUT /price） */
  price: number;
  /** 封面外链（media_info.poster） */
  posterUrl: string | null;
  /** MediaInfo 全文（media_info.mediainfo） */
  mediainfo: string | null;
  /** 当前多维属性值（B2 六类型）：kind → { dict_id, values[] }，只带初值 */
  sections: Record<string, { dict_id: number | null; values?: string[] }>;
  secKinds: SectionKindMeta[];
  secDict: Record<string, { id: number; name: string }[]>;
  cats: { id: number; name: string; parent_id?: number | null }[];
  seeders?: number;
  imdbId?: string | null;
  /** 标签字典与已选（0159 P1：详情页 aggregate 已带回，编辑表单免二次请求） */
  tagDict?: TagPayload["dict"];
  tagMine?: number[];
  /** 推荐位回显（0184）：staff 专属编辑；普通用户不渲染该块 */
  posState?: number;
  posStateUntil?: string | null;
  pickType?: number;
  isStaff?: boolean;
  /** 元数据源（0184：条目输入/PT-Gen 按站型显隐，与发布页同口径） */
  metaSources?: string[];
  /** 深链（列表编辑按钮 /torrent/{id}?edit=1）自动展开 */
  autoOpen?: boolean;
}

export function TorrentManage(p: TorrentManageProps) {
  const {
    torrentId, name, smallDescr, descr, anonymous, categoryId, price,
    posterUrl, mediainfo, sections, secKinds, secDict, cats, seeders,
    imdbId, tagDict, tagMine, posState, posStateUntil, pickType,
    isStaff, metaSources, autoOpen,
  } = p;
  const { dict, currency } = useI18n();
  const router = useRouter();
  const t = dict.torrentManage2;
  const [open, setOpen] = useState(Boolean(autoOpen));
  const [fName, setFName] = useState(name);
  const [fSub, setFSub] = useState(smallDescr ?? "");
  const [fDescr, setFDescr] = useState(descr ?? "");
  const [fAnon, setFAnon] = useState(anonymous);
  const [fImdb, setFImdb] = useState(imdbId ?? "");
  // 多选维度初值反查表（kind:name → id）：详情只下发显示名
  const secNameToId: Record<string, number> = {};
  for (const [k, rows] of Object.entries(secDict)) {
    for (const r of rows) secNameToId[`${k}:${r.name}`] = r.id;
  }
  // 多维属性 + 分类联动维度（0184 拆至 parts 的 hook；fCat 一并由其持有）
  const { fCat, setFCat, fSec, setFSec, catKinds, catDict } = useEditSections(
    sections,
    secKinds,
    secDict,
    categoryId,
    secNameToId,
  );
  const [fPrice, setFPrice] = useState(price);
  const [fPoster, setFPoster] = useState(posterUrl ?? "");
  const [fMediainfo, setFMediainfo] = useState(mediainfo ?? "");
  const [fPtgenUrl, setFPtgenUrl] = useState("");
  const [ptgenBusy, setPtgenBusy] = useState(false);
  // 标签整组编辑（0159 P1）：与发布表单同交互——普通标签 checkbox；
  // official 类不出现（详情页 toggle 走 staff 口径，这里不重复实现权限分支）
  const dictRows = (tagDict ?? []).map(normTagRow);
  const [fTags, setFTags] = useState<number[]>(() =>
    (tagMine ?? []).filter((id) =>
      dictRows.some((d) => d.id === id && d.kind !== "official"),
    ),
  );
  // 推荐位（0184）：staff 专属
  const [fPos, setFPos] = useState(posState ?? 0);
  const [fPosUntil, setFPosUntil] = useState(() =>
    posStateUntil ? new Date(posStateUntil).toISOString().slice(0, 16) : "",
  );
  const [fPick, setFPick] = useState(pickType ?? 0);
  const attach = useAttachmentUpload(setFDescr);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const descrRef = useRef<HTMLTextAreaElement>(null);
  const actions = useTorrentActions({
    torrentId,
    setMsg,
    setBusy,
    onDeleted: () => router.push("/torrents"),
  });

  const metaSrc = metaSources ?? ["imdb", "douban", "bangumi", "indienova"];

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
      const body = buildEditPayload({
        fName, fSub, fDescr, fAnon, fCat, fImdb, fPoster, fMediainfo,
        fSec, catKinds, fTags, tagMine, dictRows, isStaff, fPos, fPosUntil,
        fPick,
      });
      await api.put(`/api/v1/torrents/${torrentId}`, body);
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


  return (
    <div className="flex flex-col items-start gap-2">
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          className={`${PILL} border border-line text-sky-deep`}
        >
          ✎ {t.edit}
        </button>
        {(seeders ?? 1) === 0 && (
          <button
            type="button"
            disabled={busy}
            onClick={actions.reseed}
            className={`${PILL} border border-line text-sun`}
            title={dict.reseed2.note}
          >
            🔄 {dict.reseed2.btn}
          </button>
        )}
        <button
          type="button"
          disabled={busy}
          onClick={actions.del}
          className={`${PILL} border border-tomato/60 text-tomato`}
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
          <ManageBasicFields
            dict={dict}
            fName={fName}
            setFName={setFName}
            fSub={fSub}
            setFSub={setFSub}
            fCat={fCat}
            setFCat={setFCat}
            cats={cats}
            catKinds={catKinds}
            catDict={catDict}
            fSec={fSec}
            setFSec={setFSec}
            fPoster={fPoster}
            setFPoster={setFPoster}
            fPrice={fPrice}
            setFPrice={setFPrice}
            currency={currency}
          />
          <ManageMetaFields
            dict={dict}
            metaSrc={metaSrc}
            fPtgenUrl={fPtgenUrl}
            setFPtgenUrl={setFPtgenUrl}
            ptgenBusy={ptgenBusy}
            onGen={() => void gen()}
            fMediainfo={fMediainfo}
            setFMediainfo={setFMediainfo}
            fImdb={fImdb}
            setFImdb={setFImdb}
            attach={attach}
          />
          {/* 简介与 BBCode 工具条（0173 复用发布页组件） */}
          <UploadDescrBlock
            descr={fDescr}
            setDescr={setFDescr}
            descrRef={descrRef}
          />
          <label className="flex items-center gap-2 text-xs">
            <input
              type="checkbox"
              checked={fAnon}
              onChange={(e) => setFAnon(e.target.checked)}
            />
            {t.fieldAnonymous}
          </label>
          <ManageTagsBlock
            tagDict={tagDict}
            fTags={fTags}
            setFTags={setFTags}
          />
          {isStaff && (
            <ManagePickBlock
              fPos={fPos}
              setFPos={setFPos}
              fPosUntil={fPosUntil}
              setFPosUntil={setFPosUntil}
              fPick={fPick}
              setFPick={setFPick}
            />
          )}
          <div className="flex items-center gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={save}
              className={`${PILL} bg-sky-deep px-5 text-white`}
            >
              {t.save}
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => setOpen(false)}
              className={`${PILL} border border-line text-sub`}
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

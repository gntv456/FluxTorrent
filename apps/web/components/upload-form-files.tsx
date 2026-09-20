"use client";

import type { RefObject } from "react";
import { useState } from "react";
import { api } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { FormRow, fieldCls } from "@/components/upload-form-parts";

/** 发布表单·文件与 NFO / 附件块（从 upload-form.tsx 按域拆出，300 行门禁）：
 *  种子文件选择、标题/副标题、IMDb 与 PT-Gen、NFO、图床附件、MediaInfo、
 *  封面 URL、价格。fileRef/nfoRef/fileName 由主表单传入（submit 读取）；
 *  descr 同理（PT-Gen 生成与图床插入都会追加进同一段简介）。 */

export function UploadFilesBlock({
  fileRef,
  nfoRef,
  fileName,
  setFileName,
  name,
  setName,
  smallDescr,
  setSmallDescr,
  imdb,
  setImdb,
  ptgenUrl,
  setPtgenUrl,
  ptgenBusy,
  setPtgenBusy,
  setDescr,
  mediainfo,
  setMediainfo,
  poster,
  setPoster,
  price,
  setPrice,
  setMsg,
  metaSources,
}: {
  fileRef: RefObject<HTMLInputElement | null>;
  nfoRef: RefObject<HTMLInputElement | null>;
  fileName: string;
  setFileName: (v: string) => void;
  name: string;
  setName: (v: string) => void;
  smallDescr: string;
  setSmallDescr: (v: string) => void;
  imdb: string;
  setImdb: (v: string) => void;
  ptgenUrl: string;
  setPtgenUrl: (v: string) => void;
  ptgenBusy: boolean;
  setPtgenBusy: (b: boolean) => void;
  setDescr: (v: string | ((prev: string) => string)) => void;
  mediainfo: string;
  setMediainfo: (v: string) => void;
  poster: string;
  setPoster: (v: string) => void;
  price: number;
  setPrice: (v: number) => void;
  setMsg: (m: string | null) => void;
  metaSources: string[];
}) {
  const { dict, currency } = useI18n();
  const [nfoName, setNfoName] = useState("");
  const [attachBusy, setAttachBusy] = useState(false);
  const [attachMsg, setAttachMsg] = useState<string | null>(null);

  /** 图床上传（0100）：POST /attachments，成功后把 [img]URL[/img] 追加进简介 */
  async function uploadAttachment(file: File) {
    setAttachBusy(true);
    setAttachMsg(null);
    try {
      const fd = new FormData();
      fd.append("file", file);
      // 凭证由 HttpOnly flux_token cookie 自动携带（P1 收敛，token 不进 JS）
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
      setDescr(
        (prev) => (prev.trim() ? `${prev.trim()}

` : "") + tag,
      );
      setAttachMsg(j.data.deduplicated ? "秒传成功（服务器已有同文件）" : "上传成功，已插入简介");
    } catch (e) {
      setAttachMsg(e instanceof Error ? e.message : "上传失败");
    } finally {
      setAttachBusy(false);
    }
  }

  async function genDescr() {
    const url = ptgenUrl.trim();
    if (!url || ptgenBusy) return;
    setPtgenBusy(true);
    try {
      const r = await api.get<{ name: string; descr: string }>(
        `/api/v1/ptgen?url=${encodeURIComponent(url)}`,
      );
      setDescr((prev) =>
        (prev.trim() ? `${prev.trim()}\n\n` : "") + r.descr,
      );
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    } finally {
      setPtgenBusy(false);
    }
  }

  return (
    <>
      <FormRow label={dict.upload.fileLabel}>
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
        </div>
      </FormRow>
      <FormRow label={dict.upload.titleName ?? "标题"}>
        <div className="flex flex-col gap-1">
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={dict.upload.nameHint ?? "不填将使用种子文件名"}
            maxLength={200}
            className={fieldCls}
          />
        </div>
      </FormRow>
      <FormRow label={dict.upload.smallDescr}>
        <input
          type="text"
          value={smallDescr}
          onChange={(e) => setSmallDescr(e.target.value)}
          placeholder={dict.upload.smallDescrPlaceholder}
          maxLength={120}
          className={fieldCls}
        />
      </FormRow>
      {metaSources.includes("imdb") && (
        <FormRow label={dict.upload.imdb ?? "IMDb 链接"}>
          <div className="flex flex-col gap-1">
            <input
              type="url"
              value={imdb}
              onChange={(e) => setImdb(e.target.value)}
              placeholder="https://www.imdb.com/title/tt0468569/"
              maxLength={300}
              className={fieldCls}
            />
            <span className="text-xs text-sub">
              {dict.upload.imdbHint ?? "来自 IMDb 的条目链接；用于详情页展示与搜索区「IMDb」"}
            </span>
          </div>
        </FormRow>
      )}
      {metaSources.length > 0 && (
        <FormRow label={dict.upload.ptgen ?? "PT-Gen"}>
          <div className="flex flex-col gap-1">
            <div className="flex flex-wrap items-center gap-2">
              <input
                type="url"
                value={ptgenUrl}
                onChange={(e) => setPtgenUrl(e.target.value)}
                placeholder={dict.upload.ptgenPlaceholder ?? `粘贴 ${metaSources.join(" / ")} 链接`}
                maxLength={300}
                className="min-h-[38px] flex-1 rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 text-sm text-ink outline-none focus:border-[var(--baozi-orange)]"
              />
              <button
                type="button"
                disabled={ptgenBusy || !ptgenUrl.trim()}
                onClick={genDescr}
                className="min-h-[38px] rounded-[10px] border border-[var(--baozi-line)] px-4 text-sm font-bold text-ink hover:border-[var(--baozi-orange)] disabled:opacity-50"
              >
                {ptgenBusy ? (dict.upload.ptgenBusy ?? "生成中…") : (dict.upload.ptgenBtn ?? "生成简介")}
              </button>
            </div>
            <span className="text-xs text-sub">
              {dict.upload.ptgenHint ?? "自动拉取条目信息追加到下方简介（服务端代理，可重复追加）"}
            </span>
          </div>
        </FormRow>
      )}
      <FormRow label={dict.upload.nfo ?? "NFO 文件"}>
        <div className="flex flex-col gap-1">
          <label className="flex min-h-[48px] cursor-pointer items-center justify-center rounded-[var(--r-sm)] border border-dashed border-[var(--baozi-line)] bg-[var(--head-b)] px-3 text-sm text-[var(--text-body)] hover:border-[var(--baozi-orange)]">
            <input
              ref={nfoRef}
              type="file"
              accept=".nfo,text/plain"
              className="sr-only"
              onChange={(e) => setNfoName(e.target.files?.[0]?.name ?? "")}
            />
            {nfoName ? `📄 ${nfoName}` : (dict.upload.nfoHint ?? "可选；经典 NFO 字符画支持（CP437 / UTF-8 均可）")}
          </label>
        </div>
      </FormRow>
      <FormRow label={dict.upload.attachLabel ?? "附件 / 截图"}>
        <div className="flex flex-col gap-1">
          <label className="flex min-h-[44px] cursor-pointer items-center justify-center rounded-[var(--r-sm)] border border-dashed border-[var(--baozi-line)] bg-[var(--head-b)] px-3 text-sm text-[var(--text-body)] hover:border-[var(--baozi-orange)]">
            <input
              type="file"
              className="sr-only"
              accept="image/png,image/jpeg,image/gif,image/webp,image/avif,application/pdf,text/plain"
              disabled={attachBusy}
              onChange={(e) => {
                const f = e.target.files?.[0];
                if (f) void uploadAttachment(f);
                e.currentTarget.value = "";
              }}
            />
            {attachBusy
              ? "上传中…"
              : (dict.upload.attachHint ?? "可选；png/jpg/gif/webp/avif/pdf/txt ≤ 8MiB，上传后自动插入简介")}
          </label>
          {attachMsg && <p className="text-xs text-sky-deep">{attachMsg}</p>}
        </div>
      </FormRow>
      <FormRow label={dict.upload.mediainfoLabel ?? "MediaInfo"}>
        <textarea
          rows={4}
          value={mediainfo}
          onChange={(e) => setMediainfo(e.target.value)}
          placeholder={dict.upload.mediainfoHint ?? "可选；粘贴 MediaInfo 摘要，详情页折叠展示"}
          className="w-full rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] p-2 font-mono text-xs"
          maxLength={60000}
        />
      </FormRow>
      <FormRow label={dict.upload.price ?? "价格"}>
        <div className="flex flex-col gap-1">
          <div className="flex flex-wrap items-center gap-2">
            <input
              type="number"
              min={0}
              max={1000000}
              value={price}
              onChange={(e) => setPrice(Number(e.target.value))}
              className="min-h-[38px] w-36 rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 text-sm text-ink outline-none focus:border-[var(--baozi-orange)]"
            />
            <span className="text-sm text-sub">
              {currency}（0 = 免费，最大 1000000）
            </span>
          </div>
          <span className="text-xs text-sub">
            {dict.upload.priceHint ??
              "下载者首次下载时支付，重复下载不再扣费；税率 30%，税入当月魔法池"}
          </span>
        </div>
      </FormRow>
      <FormRow label={dict.upload.poster ?? "封面图 URL"}>
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
        </div>
      </FormRow>
    </>
  );
}

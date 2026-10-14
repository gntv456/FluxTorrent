"use client";

import { useEffect, useRef, useState } from "react";
import { api, rawFetchHelpers } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { appendMeta, packSections, rejectReason } from "@/lib/upload-fields";
import { fmt } from "@/i18n/config";
import { FormRow } from "@/components/upload-form-parts";
import { UploadFilesBlock } from "@/components/upload-form-files";
import { UploadDescrBlock } from "@/components/upload-form-descr";
import { UploadQualityBlock } from "@/components/upload-form-quality";
import { FieldErrors, fieldsFrom } from "@/components/ui/form-field";
import { SubmitButton } from "@/components/ui/submit-button";
import type { SectionKindMeta } from "@/components/admin-sections-shared";
import type { SiteProfile } from "@/lib/site-profile";

export type { SectionKindMeta };

/** 分类项 = /site-profile 的 categories 元素（含 0188 层级 parent_id，
 *  显示名由 catPath 拼「父 › 子」；类型不再另立一份，防两处漂移） */
export type ProfileCat = SiteProfile["categories"][number];
export interface SectionDictRow {
  id: number;
  kind: string;
  name: string;
  sort: number;
  mode_id: number | null;
}

/** 发布表单（NexusPHP 经典 rowhead/rowfollow 表格布局；分类与质量维度全部站点数据驱动）。
 *  按域拆出（300 行门禁）：upload-form-parts.tsx（共用行/样式）、
 *  upload-form-files.tsx（文件与 NFO/附件/元数据）、upload-form-descr.tsx
 *  （简介与 BBCode 工具条）、upload-form-quality.tsx（分类/质量/标签/推荐）；
 *  本文件保留数据源加载、状态与提交逻辑。 */
export function UploadForm() {
  const { dict, currency } = useI18n();
  // 分类/元数据源以 /site-profile 为准；tmdb（0358）为档案缺省兜底第五源
  const [profileCats, setProfileCats] = useState<ProfileCat[] | null>(null);
  const [metaSources, setMetaSources] = useState<string[]>([
    "imdb", "douban", "bangumi",
    "indienova", "mediainfo", "tmdb",
  ]);
  useEffect(() => {
    api
      .get<{ categories: ProfileCat[]; metadata_sources?: string[] }>(
        "/api/v1/site-profile",
      )
      .then((p) => {
        setProfileCats(p.categories);
        if (p.metadata_sources) setMetaSources(p.metadata_sources);
      })
      .catch(() => setProfileCats([]));
  }, []);
  const fileRef = useRef<HTMLInputElement>(null);
  const nfoRef = useRef<HTMLInputElement>(null);
  const descrRef = useRef<HTMLTextAreaElement>(null);
  const [fileName, setFileName] = useState("");
  const [name, setName] = useState("");
  const [imdb, setImdb] = useState("");
  const [ptgenUrl, setPtgenUrl] = useState("");
  const [ptgenBusy, setPtgenBusy] = useState(false);
  const [categoryId, setCategoryId] = useState(0);
  // 默认分类不硬编码 2（P2-11）：换包后 id=2 可能不存在。统一取档案
  // 首个分类；初值 0 仅作档案到位前的占位。
  useEffect(() => {
    if (!profileCats?.length) return;
    setCategoryId((cur) =>
      cur !== 0 && profileCats.some((c) => c.id === cur)
        ? cur
        : profileCats[0].id,
    );
  }, [profileCats]);
  const [smallDescr, setSmallDescr] = useState("");
  const [descr, setDescr] = useState("");
  const [poster, setPoster] = useState("");
  const [mediainfo, setMediainfo] = useState("");
  const [anonymous, setAnonymous] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  // W-18：字段级错误（后端 1002 信封；multipart 走裸 fetch 手工解）
  const [fieldErrs, setFieldErrs] = useState<Record<string, string[]>>({});
  const [busy, setBusy] = useState(false);
  // 标签（NP upload tags 口径）：启用字典多选，官方标签仅 staff（后端同口径校验）
  // 0160 P2：groups 平行数组（与 dict 下标对齐）用于按组分区
  type TagRow = { id: number; name: string; kind: string };
  const [tagDict, setTagDict] = useState<TagRow[]>([]);
  const [tagGroups, setTagGroups] = useState<string[]>([]);
  const [tagSel, setTagSel] = useState<number[]>([]);
  useEffect(() => {
    api
      .get<
        | { tags: TagRow[] | [number, string, string][]; groups?: string[] }
        | TagRow[]
        | [number, string, string][]
      >("/api/v1/tags-dict")
      .then((r) => {
        // 0160 起返回 { tags, groups }；旧形态（裸数组）兼容
        const rows = Array.isArray(r) ? r : (r.tags ?? []);
        const groups = Array.isArray(r) ? [] : (r.groups ?? []);
        setTagDict(
          rows.map((row) =>
            Array.isArray(row)
              ? { id: row[0], name: row[1], kind: row[2] }
              : row,
          ),
        );
        setTagGroups(groups);
      })
      .catch(() => {
        setTagDict([]);
        setTagGroups([]);
      });
  }, []);
  // 价格（0086 付费下载）：下载者支付，发布者得 (100-税)%，0 = 免费
  const [price, setPrice] = useState(0);
  // 推荐位（0089，NP 挑选 口径）：置顶位置/截止 + 推荐影片，需管理组权限
  const [posState, setPosState] = useState(0);
  const [posUntil, setPosUntil] = useState("");
  const [pickType, setPickType] = useState(0);
  // 质量维度（0085 可配置）：维度清单与标签来自 section_kinds，站方可自建
  const [secDict, setSecDict] = useState<Record<string, SectionDictRow[]>>({});
  const [secKinds, setSecKinds] = useState<SectionKindMeta[]>([]);
  const [secVals, setSecVals] = useState<Record<string, string>>({});
  useEffect(() => {
    // 归属模式决定「这个分类能填哪些维度」（mode_id → show_*），
    // 换分类必须重取维度清单，不能拿首次结果用到底
    api
      .get<Record<string, unknown>>(
        `/api/v1/section-dict?category_id=${categoryId}`,
      )
      .then((d) => {
        const ks = (d.kinds as SectionKindMeta[] | undefined) ?? [];
        setSecKinds(ks);
        const rest: Record<string, SectionDictRow[]> = {};
        for (const [k, v] of Object.entries(d)) {
          if (k !== "kinds" && k !== "modes") rest[k] = v as SectionDictRow[];
        }
        setSecDict(rest);
        // 被模式隐藏/换批后不存在的字典项：残留取值不能再被提交
        setSecVals((prev) => {
          const kept: Record<string, string> = {};
          for (const [k, v] of Object.entries(prev)) {
            if (v && (rest[k] ?? []).some((r) => String(r.id) === v)) {
              kept[k] = v;
            }
          }
          return kept;
        });
      })
      .catch(() => setSecDict({}));
  }, [categoryId]);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const file = fileRef.current?.files?.[0];
    if (!file) {
      setMsg(dict.upload.chooseFile);
      return;
    }
    // 媒介为 NOT NULL 外键（media.id），必须选一项，不能留空
    setBusy(true);
    setMsg(null);
    setFieldErrs({});
    try {
      const form = new FormData();
      form.append("file", file);
      const nfoFile = nfoRef.current?.files?.[0];
      if (nfoFile) form.append("nfo", nfoFile);
      // 0288：元数据走 multipart 文本字段（服务端仍兼容 query string 老通道）。
      // 塞 URL 时简介上限由 HTTP 请求行决定：实测 8192 汉字即 400 空响应体。
      appendMeta(form, {
        category_id: categoryId,
        anonymous,
        name,
        imdb,
        price,
        small_descr: smallDescr,
        descr,
        poster,
        mediainfo,
        sections: packSections(secVals, secKinds),
        tags: tagSel,
        pos_state: posState,
        pos_state_until: posUntil ? new Date(posUntil).toISOString() : "",
        pick_type: pickType,
      });
      // 同源相对路径走 Next rewrites 转发（与 api-client 同口径），避免依赖发布端口
      const base = rawFetchHelpers.base();
      const res = await fetch(`${base}/api/v1/torrents`, {
        method: "POST",
        body: form,
      });
      const body = await res.json();
      if (body.code !== 0) {
        // W-18：1002 字段级信封 → FieldErrors 按字段渲染；其余按码走字典
        const errs = fieldsFrom(body.data);
        if (body.code === 1002 && Object.keys(errs).length > 0) {
          const asArr = Object.entries(errs).map(([f, v]) => [
            f,
            Array.isArray(v) ? v : [v],
          ]);
          setFieldErrs(Object.fromEntries(asArr));
        } else {
          setMsg(rejectReason(body, dict));
        }
      } else {
        // 发布成功直接进详情页预览（0159）：待审种对本人可见
        const newId = body.data.id;
        // 0284 P1-4：同源提示（group_suggest.same_source）优先于普通成功文案
        const gs = body.data.group_suggest;
        if (gs?.same_source) {
          setMsg(
            fmt(dict.upload.sameSource, {
              id: gs.torrent_id,
              name: gs.name ?? "",
            }),
          );
        } else {
          setMsg(fmt(dict.upload.success, { id: newId }));
        }
        setTimeout(() => {
          window.location.href = `/torrent/${newId}`;
        }, 900);
      }
    } catch {
      setMsg(dict.upload.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
    <form onSubmit={submit} id="upload-form">
      <div className="baozi-wide-table-scroll">
      <table className="nexus-table nexus-form">
        <thead>
          <tr>
            <td className="colhead" colSpan={2}>{dict.upload.title}</td>
          </tr>
        </thead>
        <tbody>
          <UploadFilesBlock
            fileRef={fileRef}
            nfoRef={nfoRef}
            fileName={fileName}
            setFileName={setFileName}
            name={name}
            setName={setName}
            smallDescr={smallDescr}
            setSmallDescr={setSmallDescr}
            imdb={imdb}
            setImdb={setImdb}
            ptgenUrl={ptgenUrl}
            setPtgenUrl={setPtgenUrl}
            ptgenBusy={ptgenBusy}
            setPtgenBusy={setPtgenBusy}
            setDescr={setDescr}
            mediainfo={mediainfo}
            setMediainfo={setMediainfo}
            poster={poster}
            setPoster={setPoster}
            price={price}
            setPrice={setPrice}
            setMsg={setMsg}
            metaSources={metaSources}
          />
          <UploadDescrBlock
            descr={descr}
            setDescr={setDescr}
            descrRef={descrRef}
          />
          <UploadQualityBlock
            profileCats={profileCats}
            categoryId={categoryId}
            setCategoryId={setCategoryId}
            secKinds={secKinds}
            secDict={secDict}
            secVals={secVals}
            setSecVals={setSecVals}
            tagDict={tagDict}
            tagGroups={tagGroups}
            tagSel={tagSel}
            setTagSel={setTagSel}
            posState={posState}
            setPosState={setPosState}
            posUntil={posUntil}
            setPosUntil={setPosUntil}
            pickType={pickType}
            setPickType={setPickType}
          />
          <FormRow label={dict.upload.anonymous}>
            <label className="flex cursor-pointer items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={anonymous}
                onChange={(e) => setAnonymous(e.target.checked)}
                className="h-4 w-4 accent-[var(--baozi-orange)]"
              />
              {dict.upload.anonymous}
            </label>
          </FormRow>
          <tr>
            <td className="rowfollow" colSpan={2}>
              {/* W-18：字段级错误汇总（1002 信封）+ SubmitButton 生命周期 */}
              <div className="flex flex-wrap items-center gap-3">
                <FieldErrors fields={fieldErrs} />
                <SubmitButton
                  busy={busy}
                  label={dict.upload.submit}
                  busyLabel={dict.upload.busy}
                />
                {msg && (
                  <p role="status" className="text-sm text-ink">{msg}</p>
                )}
              </div>
            </td>
          </tr>
        </tbody>
      </table>
      </div>
      </form>
      {/* M5.5：<md 固定提交条（长表单底部可达；≥md 不渲染；同源 busy） */}
      <button
        type="submit"
        form="upload-form"
        disabled={busy}
        className="up-submitbar md:hidden"
      >
        {busy ? dict.upload.busy : dict.upload.submit}
      </button>
    </>
  );
}

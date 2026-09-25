"use client";

import { useEffect, useRef, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { FormRow } from "@/components/upload-form-parts";
import { UploadFilesBlock } from "@/components/upload-form-files";
import { UploadDescrBlock } from "@/components/upload-form-descr";
import { UploadQualityBlock } from "@/components/upload-form-quality";
import type { SectionKindMeta } from "@/components/admin-sections-shared";

export type { SectionKindMeta };

export interface ProfileCat {
  id: number;
  name: string;
}
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
  // 分类/元数据源以 /site-profile 为准（类型包可切换）；字典仅兜底分类
  const [profileCats, setProfileCats] = useState<ProfileCat[] | null>(null);
  const [metaSources, setMetaSources] = useState<string[]>([
    "imdb",
    "douban",
    "bangumi",
    "indienova",
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
  const [categoryId, setCategoryId] = useState(2);
  // 默认分类不再硬编码 2：切换站型包后 id=2 可能压根不存在（有的包只有 3 个分类），
  // 那会把非法 category_id 提交上去。档案到位后校正一次。
  useEffect(() => {
    if (!profileCats?.length) return;
    setCategoryId((cur) =>
      profileCats.some((c) => c.id === cur) ? cur : profileCats[0].id,
    );
  }, [profileCats]);
  const [smallDescr, setSmallDescr] = useState("");
  const [descr, setDescr] = useState("");
  const [poster, setPoster] = useState("");
  const [mediainfo, setMediainfo] = useState("");
  const [anonymous, setAnonymous] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // 标签（NP upload tags 口径）：启用字典多选，官方标签仅 staff（后端同口径校验）
  // 0160 P2：groups 平行数组（与 dict 下标对齐）用于按组分区
  const [tagDict, setTagDict] = useState<
    { id: number; name: string; kind: string }[]
  >([]);
  const [tagGroups, setTagGroups] = useState<string[]>([]);
  const [tagSel, setTagSel] = useState<number[]>([]);
  useEffect(() => {
    api
      .get<
        | {
            tags:
              | { id: number; name: string; kind: string }[]
              | [number, string, string][];
            groups?: string[];
          }
        | { id: number; name: string; kind: string }[]
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
    // 归属模式决定「这个分类能填哪些维度」（categories.mode_id → show_*），
    // 所以换分类必须重取维度清单，不能再拿首次加载的结果用到底
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
        // 被模式隐藏的维度、或换批后不再存在的字典项：残留取值不能再被提交
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
    try {
      const form = new FormData();
      form.append("file", file);
      const nfoFile = nfoRef.current?.files?.[0];
      if (nfoFile) form.append("nfo", nfoFile);
      const qs = new URLSearchParams({
        category_id: String(categoryId),
        anonymous: String(anonymous),
      });
      if (name.trim()) qs.set("name", name.trim());
      if (imdb.trim()) qs.set("imdb", imdb.trim());
      if (price > 0)
        qs.set("price", String(Math.min(1_000_000, Math.max(0, price))));
      if (smallDescr.trim()) qs.set("small_descr", smallDescr.trim());
      if (descr.trim()) qs.set("descr", descr.trim());
      if (poster.trim()) qs.set("poster", poster.trim());
      if (mediainfo.trim()) qs.set("mediainfo", mediainfo.trim());
      // 多维属性打包（B2 六类型）：枚举维度发整数（旧格式，后端零改动兼容），
      // 自由值维度按 field_type 发对象 {"text":…}/{"number":…}/{"date":…}/{"bool":…}；
      // multiselect 发 {"dict_ids":[…]}。后端按**值的 JSON 类型**分派。
      const sections: Record<string, unknown> = {};
      for (const [k, v] of Object.entries(secVals)) {
        if (!v) continue;
        const def = secKinds.find((x) => x.kind === k);
        const type = def?.field_type ?? "select";
        if (type === "select") {
          const n = Number(v);
          if (n > 0) sections[k] = n;
        } else if (type === "multiselect") {
          const ids = v
            .split(",")
            .map((s) => Number(s.trim()))
            .filter((n) => n > 0);
          if (ids.length > 0) sections[k] = { dict_ids: ids };
        } else if (type === "number") {
          const n = Number(v);
          if (!Number.isNaN(n)) sections[k] = { number: n };
        } else if (type === "bool") {
          sections[k] = { bool: v === "true" };
        } else if (type === "date") {
          if (v.trim()) sections[k] = { date: v.trim() };
        } else {
          if (v.trim()) sections[k] = { text: v.trim() };
        }
      }
      if (Object.keys(sections).length > 0)
        qs.set("sections", JSON.stringify(sections));
      // 标签 / 推荐位（挑选）
      if (tagSel.length > 0) qs.set("tags", JSON.stringify(tagSel));
      if (posState > 0) {
        qs.set("pos_state", String(posState));
        if (posUntil)
          qs.set("pos_state_until", new Date(posUntil).toISOString());
      }
      if (pickType > 0) qs.set("pick_type", String(pickType));
      // 同源相对路径走 Next rewrites 转发（与 api-client 同口径），避免依赖发布端口
      const base = process.env.NEXT_PUBLIC_API_URL ?? "";
      const res = await fetch(`${base}/api/v1/torrents?${qs}`, {
        method: "POST",
        body: form,
      });
      const body = await res.json();
      if (body.code !== 0) {
        setMsg(dict.errors[body.code] ?? body.message ?? dict.upload.fail);
      } else {
        // 发布成功直接进详情页预览（0159 用户反馈）：仅提示不跳转时用户
        // 不知道去哪看；待审种对本人可见（get_torrent 口径）
        const newId = body.data.id;
        setMsg(fmt(dict.upload.success, { id: newId }));
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

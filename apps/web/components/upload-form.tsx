"use client";

import { useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

interface ProfileCat { id: number; name: string }
interface SectionDictRow { id: number; kind: string; name: string; sort: number; mode_id: number | null }

/** 兜底维度标签（/section-dict 不可用时；正常路径标签来自 section_kinds 站方可配置） */
const SECTION_KINDS: [string, string][] = [
  ["media", "媒介"],
  ["grades", "学段"],
  ["editions", "版本"],
  ["codec", "编码"],
  ["audio_codec", "音频编码"],
  ["standard", "规格"],
  ["team", "制作组"],
  ["source", "来源"],
  ["processing", "处理工艺"],
];

interface SectionKindMeta { kind: string; label: string; sort: number }

/** 发布表单（NexusPHP 经典 rowhead/rowfollow 表格布局；分类来自站点档案，支持任意类型 PT 站） */
export function UploadForm() {
  const { dict, currency } = useI18n();
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
  const nfoRef = useRef<HTMLInputElement>(null);
  const descrRef = useRef<HTMLTextAreaElement>(null);
  const [fileName, setFileName] = useState("");
  const [nfoName, setNfoName] = useState("");
  const [name, setName] = useState("");
  const [imdb, setImdb] = useState("");
  const [ptgenUrl, setPtgenUrl] = useState("");
  const [ptgenBusy, setPtgenBusy] = useState(false);
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
  // 标签（NP upload tags 口径）：启用字典多选，官方标签仅 staff（后端同口径校验）
  const [tagDict, setTagDict] = useState<{ id: number; name: string; kind: string }[]>([]);
  const [tagSel, setTagSel] = useState<number[]>([]);
  useEffect(() => {
    api.get<{ id: number; name: string; kind: string }[] | [number, string, string][]>(
      "/api/v1/tags-dict",
    )
      .then((rows) =>
        setTagDict(
          rows.map((r) => (Array.isArray(r) ? { id: r[0], name: r[1], kind: r[2] } : r)),
        ),
      )
      .catch(() => setTagDict([]));
  }, []);
  // 挑选（促销位）：发布时直接设置单种促销，需 torrent.set_price 权限
  const [promoKind, setPromoKind] = useState("");
  const [promoHours, setPromoHours] = useState(48);
  // 价格（0086 付费下载）：下载者支付，发布者得 (100-税)%，0 = 免费
  const [price, setPrice] = useState(0);
  // 质量维度（0085 可配置）：维度清单与标签来自 section_kinds，站方可自建
  const [secDict, setSecDict] = useState<Record<string, SectionDictRow[]>>({});
  const [secKinds, setSecKinds] = useState<SectionKindMeta[]>([]);
  const [secVals, setSecVals] = useState<Record<string, string>>({});
  useEffect(() => {
    api.get<Record<string, unknown>>("/api/v1/section-dict")
      .then((d) => {
        setSecKinds((d.kinds as SectionKindMeta[] | undefined) ?? []);
        const rest: Record<string, SectionDictRow[]> = {};
        for (const [k, v] of Object.entries(d)) {
          if (k !== "kinds" && k !== "modes") rest[k] = v as SectionDictRow[];
        }
        setSecDict(rest);
      })
      .catch(() => setSecDict({}));
  }, []);

  async function genDescr() {
    const url = ptgenUrl.trim();
    if (!url || ptgenBusy) return;
    setPtgenBusy(true);
    try {
      const r = await api.get<{ name: string; descr: string }>(
        `/api/v1/ptgen?url=${encodeURIComponent(url)}`,
      );
      setDescr((prev) => (prev.trim() ? `${prev.trim()}\n\n` : "") + r.descr);
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    } finally {
      setPtgenBusy(false);
    }
  }

  // BBCode 工具条（NP bbcode 口径）：包住选区 / 插入
  function bbWrap(open: string, close: string, ph = "") {
    const el = descrRef.current;
    const s = el?.selectionStart ?? descr.length;
    const e = el?.selectionEnd ?? descr.length;
    const sel = descr.slice(s, e) || ph;
    setDescr(descr.slice(0, s) + open + sel + close + descr.slice(e));
    requestAnimationFrame(() => {
      if (!el) return;
      el.focus();
      el.selectionStart = s + open.length;
      el.selectionEnd = s + open.length + sel.length;
    });
  }
  function bbInsert(txt: string) {
    const el = descrRef.current;
    const s = el?.selectionStart ?? descr.length;
    const e = el?.selectionEnd ?? s;
    setDescr(descr.slice(0, s) + txt + descr.slice(e));
    requestAnimationFrame(() => {
      if (!el) return;
      el.focus();
      el.selectionStart = el.selectionEnd = s + txt.length;
    });
  }
  const bbBtn =
    "min-h-[28px] min-w-[32px] rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-1.5 text-xs font-bold text-ink hover:border-[var(--baozi-orange)]";
  const bbSelect =
    "min-h-[28px] rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-1 text-xs text-ink outline-none focus:border-[var(--baozi-orange)]";
  const BB_EMOJIS = ["😄", "😂", "🥰", "😮", "😭", "😅", "😡", "👍", "🙏", "🎉", "🔥", "❤️", "🤔", "💯", "🍺"];

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const file = fileRef.current?.files?.[0];
    if (!file) {
      setMsg(dict.upload.chooseFile);
      return;
    }
    // 媒介为 NOT NULL 外键（media.id），必须选一项，不能留空
    if (!mediumId) {
      setMsg(dict.upload.mediumRequired ?? "请选择媒介");
      return;
    }
    setBusy(true);
    setMsg(null);
    try {
      const form = new FormData();
      form.append("file", file);
      const nfoFile = nfoRef.current?.files?.[0];
      if (nfoFile) form.append("nfo", nfoFile);
      const qs = new URLSearchParams({
        category_id: String(categoryId),
        medium_id: String(mediumId),
        anonymous: String(anonymous),
      });
      if (name.trim()) qs.set("name", name.trim());
      if (imdb.trim()) qs.set("imdb", imdb.trim());
      if (price > 0) qs.set("price", String(Math.min(1_000_000, Math.max(0, price))));
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
      // 标签 / 促销（挑选）
      if (tagSel.length > 0) qs.set("tags", JSON.stringify(tagSel));
      if (promoKind) {
        qs.set("promo_kind", promoKind);
        qs.set("promo_hours", String(Math.min(720, Math.max(1, promoHours || 48))));
      }
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

  // 质量维度渲染清单：API可用 → 全由 section_kinds/section_dict 驱动；不可用 → 三维静态兜底
  // legacy 三维（media/grades/editions）落实体列，其余进 sections JSON
  const kindDefs = (
    secKinds.length > 0
      ? secKinds.map((k) => ({ kind: k.kind, label: k.label }))
      : SECTION_KINDS.map(([kind, label]) => ({ kind, label }))
  )
    .map((k) => ({ ...k, opts: (secDict[k.kind] ?? []).map((d) => ({ v: d.id, label: d.name })) }))
    .map((k) =>
      k.opts.length === 0 && secKinds.length === 0
        ? {
            ...k,
            opts:
              k.kind === "media"
                ? media.map((label, i) => ({ v: i + 1, label }))
                : k.kind === "grades"
                  ? grades.map((label, i) => ({ v: i, label }))
                  : k.kind === "editions"
                    ? editions.map((label, i) => ({ v: i + 1, label }))
                    : [],
          }
        : k,
    )
    .filter((k) => k.opts.length > 0);
  const kindVal = (kind: string) =>
    kind === "media"
      ? String(mediumId)
      : kind === "grades"
        ? gradeId
        : kind === "editions"
          ? editionId
          : (secVals[kind] ?? "");
  const setKindVal = (kind: string, v: string) => {
    if (kind === "media") setMediumId(Number(v));
    else if (kind === "grades") setGradeId(v);
    else if (kind === "editions") setEditionId(v);
    else setSecVals((prev) => ({ ...prev, [kind]: v }));
  };

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
            dict.upload.titleName ?? "标题",
            <div className="flex flex-col gap-1">
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={dict.upload.nameHint ?? "不填将使用种子文件名"}
                maxLength={200}
                className={fieldCls}
              />
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
            dict.upload.imdb ?? "IMDb 链接",
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
            </div>,
          )}
          {row(
            dict.upload.ptgen ?? "PT-Gen",
            <div className="flex flex-col gap-1">
              <div className="flex flex-wrap items-center gap-2">
                <input
                  type="url"
                  value={ptgenUrl}
                  onChange={(e) => setPtgenUrl(e.target.value)}
                  placeholder={dict.upload.ptgenPlaceholder ?? "粘贴 imdb / douban / bangumi / indienova 链接"}
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
            </div>,
          )}
          {row(
            dict.upload.nfo ?? "NFO 文件",
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
            </div>,
          )}
          {row(
            dict.upload.price ?? "价格",
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
            </div>,
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
              <div className="flex flex-wrap items-center gap-1">
                <select
                  value=""
                  onChange={(e) => {
                    if (e.target.value) bbWrap(`[color=${e.target.value}]`, "[/color]");
                  }}
                  className={bbSelect}
                  title={dict.upload.bbColor ?? "颜色"}
                >
                  <option value="">{dict.upload.bbColor ?? "颜色"}</option>
                  <option value="#e02020">🔴 红</option>
                  <option value="#f59e0b">🟠 橙</option>
                  <option value="#16a34a">🟢 绿</option>
                  <option value="#2563eb">🔵 蓝</option>
                  <option value="#9333ea">🟣 紫</option>
                  <option value="#6b7280">⚪ 灰</option>
                </select>
                <select
                  value=""
                  onChange={(e) => {
                    if (e.target.value) bbWrap(`[font=${e.target.value}]`, "[/font]");
                  }}
                  className={bbSelect}
                  title={dict.upload.bbFont ?? "字体"}
                >
                  <option value="">{dict.upload.bbFont ?? "字体"}</option>
                  <option value="SimSun">宋体</option>
                  <option value="KaiTi">楷体</option>
                  <option value="SimHei">黑体</option>
                  <option value="serif">Serif</option>
                  <option value="monospace">等宽</option>
                </select>
                <select
                  value=""
                  onChange={(e) => {
                    if (e.target.value) bbWrap(`[size=${e.target.value}]`, "[/size]");
                  }}
                  className={bbSelect}
                  title={dict.upload.bbSize ?? "字号"}
                >
                  <option value="">{dict.upload.bbSize ?? "字号"}</option>
                  <option value="1">小</option>
                  <option value="3">中</option>
                  <option value="5">大</option>
                </select>
                <button type="button" className={`${bbBtn} font-black`} onClick={() => bbWrap("[b]", "[/b]")} title="Bold">B</button>
                <button type="button" className={`${bbBtn} italic`} onClick={() => bbWrap("[i]", "[/i]")} title="Italic">I</button>
                <button type="button" className={`${bbBtn} underline`} onClick={() => bbWrap("[u]", "[/u]")} title="Underline">U</button>
                <button type="button" className={`${bbBtn} line-through`} onClick={() => bbWrap("[s]", "[/s]")} title="Strikethrough">S</button>
                <button type="button" className={bbBtn} onClick={() => bbWrap("[url]", "[/url]", "https://")} title={dict.upload.bbLink ?? "链接"}>🔗</button>
                <button
                  type="button"
                  className={bbBtn}
                  title={dict.upload.bbImg ?? "图片"}
                  onClick={() => {
                    const u = window.prompt(dict.upload.bbImg ?? "图片 URL");
                    if (u && u.trim()) bbInsert(`[img]${u.trim()}[/img]`);
                  }}
                >🖼️</button>
                <button type="button" className={bbBtn} onClick={() => bbWrap("[quote]", "[/quote]")} title={dict.upload.bbQuote ?? "引用"}>❝</button>
                <button
                  type="button"
                  className={bbBtn}
                  title={dict.upload.bbCode ?? "代码 / MediaInfo"}
                  onClick={() => bbWrap("[code]", "[/code]", "MediaInfo / General / Complete name …")}
                >{"</>"}</button>
                <details className="relative">
                  <summary className={`${bbBtn} inline-flex cursor-pointer list-none items-center justify-center`} title={dict.upload.bbEmoji ?? "表情"}>😀</summary>
                  <div className="absolute z-10 mt-1 flex flex-wrap gap-1 rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] p-2 shadow-[var(--shadow-card)]">
                    {BB_EMOJIS.map((em) => (
                      <button key={em} type="button" className="text-lg hover:scale-125" onClick={() => bbInsert(em)}>
                        {em}
                      </button>
                    ))}
                  </div>
                </details>
              </div>
              <textarea
                ref={descrRef}
                value={descr}
                onChange={(e) => setDescr(e.target.value)}
                placeholder={dict.upload.descrPlaceholder}
                rows={9}
                maxLength={30000}
                className={fieldCls}
              />
              <span className="text-xs text-sub">
                {dict.upload.descrHint}（BBCode：<code>[b][i][color=][size=][url][img][quote][code]</code> 均受支持）
              </span>
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
          {kindDefs.length > 0 &&
            row(
              dict.upload.quality ?? "质量",
              <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
                {kindDefs.map(({ kind, label, opts }) => (
                  <label key={kind} className="flex items-center gap-1 text-sm">
                    <span className="whitespace-nowrap text-sub">{label}：</span>
                    <select
                      value={kindVal(kind)}
                      onChange={(e) => setKindVal(kind, e.target.value)}
                      className="min-h-[32px] rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-2 text-sm text-ink outline-none focus:border-[var(--baozi-orange)]"
                    >
                      <option value="">{dict.upload.gradeNone}</option>
                      {opts.map((o) => (
                        <option key={o.v} value={o.v}>
                          {o.label}
                        </option>
                      ))}
                    </select>
                  </label>
                ))}
              </div>,
            )}
          {tagDict.filter((t) => t.kind !== "official").length > 0 &&
            row(
              dict.upload.tags ?? "标签",
              <div className="flex flex-col gap-1">
                <div className="flex flex-wrap gap-x-4 gap-y-2">
                  {tagDict
                    .filter((t) => t.kind !== "official")
                    .map((t) => {
                      const on = tagSel.includes(t.id);
                      return (
                        <label key={t.id} className="flex cursor-pointer items-center gap-1.5 text-sm">
                          <input
                            type="checkbox"
                            checked={on}
                            onChange={() =>
                              setTagSel((prev) =>
                                on ? prev.filter((x) => x !== t.id) : [...prev, t.id],
                              )
                            }
                            className="h-4 w-4 accent-[var(--baozi-orange)]"
                          />
                          {t.name}
                        </label>
                      );
                    })}
                </div>
                <span className="text-xs text-sub">
                  {dict.upload.tagsHint ?? "可多选（≤12 个）；发布后可在详情页增删"}
                </span>
              </div>,
            )}
          {row(
            dict.upload.promo ?? "促销（挑选）",
            <div className="flex flex-col gap-1">
              <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
                <label className="flex items-center gap-1 text-sm">
                  <span className="whitespace-nowrap text-sub">{dict.upload.promoKind ?? "促销类型"}：</span>
                  <select
                    value={promoKind}
                    onChange={(e) => setPromoKind(e.target.value)}
                    className="min-h-[32px] rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-2 text-sm text-ink outline-none focus:border-[var(--baozi-orange)]"
                  >
                    <option value="">{dict.upload.promoNone ?? "不设置"}</option>
                    <option value="free">{dict.upload.promoFree ?? "免费"}</option>
                    <option value="x2">{dict.upload.promoX2 ?? "双倍上传"}</option>
                    <option value="x2free">{dict.upload.promoX2Free ?? "双倍+免费"}</option>
                    <option value="half">{dict.upload.promoHalf ?? "半价"}</option>
                    <option value="x2half">{dict.upload.promoX2Half ?? "双倍+半价"}</option>
                    <option value="p30">{dict.upload.promoP30 ?? "30% 下载"}</option>
                  </select>
                </label>
                {promoKind && (
                  <label className="flex items-center gap-1 text-sm">
                    <span className="whitespace-nowrap text-sub">{dict.upload.promoHours ?? "时长（小时）"}：</span>
                    <input
                      type="number"
                      min={1}
                      max={720}
                      value={promoHours}
                      onChange={(e) => setPromoHours(Number(e.target.value))}
                      className="min-h-[32px] w-24 rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-2 text-sm text-ink outline-none focus:border-[var(--baozi-orange)]"
                    />
                  </label>
                )}
              </div>
              <span className="text-xs text-sub">
                {dict.upload.promoHint ?? "发布者自助促销位，需促销权限；缺省 48 小时（1-720）"}
              </span>
            </div>,
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

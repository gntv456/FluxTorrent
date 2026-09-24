"use client";

import { useEffect, useMemo, useState } from "react";
import { useSearchParams } from "next/navigation";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface RssInfo {
  urls: { label: string; url: string }[];
  passkey: string;
  base: string;
}

interface ProfileCats {
  categories?: { id: number; name: string }[];
}

/** 媒介维度选项（0103：站型化后媒介在 section_dict(kind=media)，id 是 dict_id） */
interface MediaDictRow {
  id: number;
  name: string;
}

const SHOWROWS = [10, 50, 100, 200];

/** 获取 RSS 页（好学站 getrss.php 复刻）：
 *  检索分类（分类/媒介多选）+ 官种 + 付费 + 标题格式 + 每页条数 + 关键字 → 生成带 passkey 的订阅链接 */
export function RssBuilder({ loginToView }: { loginToView: string }) {
  const { dict, currency } = useI18n();
  const t = dict.getrss;
  const [info, setInfo] = useState<RssInfo | null>(null);
  const [cats, setCats] = useState<number[]>([]);
  const [media, setMedia] = useState<number[]>([]);
  const [official, setOfficial] = useState(false);
  const [paid, setPaid] = useState<"0" | "1">("0");
  const [showrows, setShowrows] = useState(50);
  const [linktype, setLinktype] = useState<"dl" | "page">("dl");
  // 种子列表「订阅当前结果」入口预填：?keyword= 带入当前搜索词
  const sp = useSearchParams();
  const [search, setSearch] = useState(sp.get("keyword") ?? "");
  const [copied, setCopied] = useState(false);
  // 0103：分类/媒介跟随站型配置（site-profile + section-dict），不再用 i18n 硬编码字典
  const [profile, setProfile] = useState<ProfileCats | null>(null);
  const [mediumOpts, setMediumOpts] = useState<MediaDictRow[]>([]);

  useEffect(() => {
    api
      .get<RssInfo>("/api/v1/rss-info")
      .then(setInfo)
      .catch(() => setInfo(null));
    api
      .get<ProfileCats>("/api/v1/site-profile")
      .then(setProfile)
      .catch(() => setProfile(null));
    api
      .get<Record<string, MediaDictRow[]>>("/api/v1/section-dict")
      .then((d) => setMediumOpts(d.media ?? []))
      .catch(() => setMediumOpts([]));
  }, []);

  // 只认站点档案下发的分类与媒介字典（id 即后端口径）；未就绪/失败就是空列表，
  // 不回落 i18n 字典——那是另一套词表，生成的链接会指向不存在的分类
  const categories = profile?.categories ?? [];
  const mediums = mediumOpts;

  const url = useMemo(() => {
    if (!info) return "";
    const qs = new URLSearchParams();
    if (cats.length > 0 && cats.length < categories.length) {
      qs.set("categories", cats.join(","));
    }
    if (media.length > 0 && media.length < mediums.length) {
      qs.set("mediums", media.join(","));
    }
    if (official) qs.set("official", "true");
    if (paid === "1") qs.set("paid", "1");
    qs.set("showrows", String(showrows));
    if (linktype === "page") qs.set("linktype", "page");
    if (search.trim()) qs.set("search", search.trim());
    const s = qs.toString();
    return `${info.base}${info.passkey}${s ? `?${s}` : ""}`;
  }, [
    info,
    cats,
    media,
    official,
    paid,
    showrows,
    linktype,
    search,
    categories.length,
    mediums.length,
  ]);

  function toggle(list: number[], id: number, set: (v: number[]) => void) {
    set(list.includes(id) ? list.filter((x) => x !== id) : [...list, id]);
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText(url);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // 剪贴板权限被拒时退化：选中输入框由用户手动复制
    }
  }

  if (!info) return <p className="text-xs text-sub">{loginToView}</p>;

  return (
    <div className="flex flex-col gap-4">
      <section className="rss-card">
        <h2 className="rss-card__title">{t.filterTitle}</h2>
        <fieldset className="rss-fieldset">
          <legend>{t.catLegend}</legend>
          <div className="rss-checks">
            {categories.map((c) => (
              <label key={c.id} className="rss-check">
                <input
                  type="checkbox"
                  checked={cats.includes(c.id)}
                  onChange={() => toggle(cats, c.id, setCats)}
                />
                {c.name}
              </label>
            ))}
          </div>
        </fieldset>
        <fieldset className="rss-fieldset">
          <legend>{t.mediumLegend}</legend>
          <div className="rss-checks">
            {mediums.map((m) => (
              <label key={m.id} className="rss-check">
                <input
                  type="checkbox"
                  checked={media.includes(m.id)}
                  onChange={() => toggle(media, m.id, setMedia)}
                />
                {m.name}
              </label>
            ))}
          </div>
        </fieldset>

        <div className="rss-rows">
          <label className="rss-row">
            <span>{t.officialOnly}</span>
            <input
              type="checkbox"
              checked={official}
              onChange={(e) => setOfficial(e.target.checked)}
            />
          </label>
          <label className="rss-row">
            <span>{t.paid.replace("{magic}", currency)}</span>
            <select
              value={paid}
              onChange={(e) => setPaid(e.target.value as "0" | "1")}
            >
              <option value="0">{t.paidAll}</option>
              <option value="1">{t.paidFree}</option>
            </select>
          </label>
          <label className="rss-row">
            <span>{t.titleFormat}</span>
            <select
              value={linktype}
              onChange={(e) => setLinktype(e.target.value as "dl" | "page")}
            >
              <option value="dl">{t.titleFull}</option>
              <option value="page">{t.titlePlain}</option>
            </select>
          </label>
          <label className="rss-row">
            <span>{t.showrows}</span>
            <select
              value={showrows}
              onChange={(e) => setShowrows(Number(e.target.value))}
            >
              {SHOWROWS.map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
            </select>
          </label>
        </div>

        <label className="rss-row rss-row--search">
          <span>{t.keyword}</span>
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t.keywordPh}
          />
          <small className="rss-hint">{t.keywordNote}</small>
        </label>
      </section>

      <section className="rss-card rss-card--result">
        <h2 className="rss-card__title">{t.resultTitle}</h2>
        <p className="rss-hint">{t.securityNote}</p>
        <div className="rss-url-row">
          <input
            type="text"
            readOnly
            value={url}
            onFocus={(e) => e.target.select()}
          />
          <button type="button" onClick={copy} className="rss-copy">
            {copied ? t.copied : t.copy}
          </button>
        </div>
        <div className="rss-presets">
          {info.urls.map((u) => (
            <label key={u.url} className="rss-preset">
              <span>{u.label}</span>
              <input
                type="text"
                readOnly
                value={u.url}
                onFocus={(e) => e.target.select()}
              />
            </label>
          ))}
        </div>
      </section>
    </div>
  );
}

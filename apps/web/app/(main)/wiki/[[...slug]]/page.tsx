import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { cache } from "react";
import { api, ApiError } from "@/lib/api-client";
import { ErrorPanel } from "@/components/ui/error-panel";
import { requireModule } from "@/components/module-gate";
import { getDict } from "@/i18n/server";
import { PANEL_BRAND, WIKI_SIDE_LINK, WIKI_TOC_LINK } from "@/lib/ui-classes";
import { hasBBCode, renderBBCode } from "@/lib/bbcode";
import { MarkdownRenderer } from "@/components/forum-markdown";
import { extractToc } from "@/components/forum-toc";
import { WikiIndex } from "@/components/wiki-index";
import "@/app/styles/wiki.css";

/**
 * Wiki 公开页（0359）：无 slug = 目录页，有 slug = 正文页。
 *
 * 0495 目录升级：分组卡片（section 列）+ 即时检索 + 正文页三栏
 * （左 = 同组页面导航 / 中 = 正文 / 右 = 本页 TOC，复用论坛帖子页
 * 的 extractToc 口径）。正文渲染双路：BBCode → renderBBCode（XSS
 * 天然免疫），否则 markdown-lite（与种子简介同口径）。
 */

interface WikiIndexItem {
  id: number;
  slug: string;
  title: string;
  section: string;
  updated_at: string;
}

interface WikiPage {
  slug: string;
  title: string;
  body: string;
  section: string;
  updated_at: string;
}

/** W-27 四态口径（同 /top）：取数失败要能诊断，不许演成「页面不存在」。
 *  旧写法两个 loader 都是裸 `catch { return null }` ⇒ 500/网络失败/4101 与
 *  「slug 真不存在」全被 `notFound()` 合成同一个 404。 */
type Load<T> = { data: T; err: null } | { data: null; err: unknown };

/** 只有「确实不存在」才走 404；其余失败交给 ErrorPanel（P-35b：业务码分不出
 *  不存在与暂时读不到，故一并看 HTTP status） */
function isMissing(e: unknown): boolean {
  return e instanceof ApiError && (e.code === 1004 || e.status === 404);
}

const loadIndex = cache(async (): Promise<Load<WikiIndexItem[]>> => {
  try {
    return { data: await api.get<WikiIndexItem[]>("/api/v1/wiki"), err: null };
  } catch (e) {
    return { data: null, err: e };
  }
});

const loadPage = cache(
  async (slug: string): Promise<Load<WikiPage>> => {
    try {
      const url = `/api/v1/wiki/${slug}`;
      return { data: await api.get<WikiPage>(url), err: null };
    } catch (e) {
      return { data: null, err: e };
    }
  },
);

/** 目录分组定义：固定四组 + 未分组兜底。顺序即展示顺序（新手 → 进阶 →
 *  制度 → 工具），与 dict.wiki.section* 键一一对应。 */
function sectionDefs(dict: Awaited<ReturnType<typeof getDict>>["dict"]) {
  const w = dict.wiki;
  return [
    { key: "start", title: w.sectionStart, desc: w.sectionStartDesc },
    { key: "site", title: w.sectionSite, desc: w.sectionSiteDesc },
    { key: "rules", title: w.sectionRules, desc: w.sectionRulesDesc },
    { key: "api", title: w.sectionApi, desc: w.sectionApiDesc },
    { key: "", title: w.sectionOther, desc: w.sectionOtherDesc },
  ];
}

export async function generateMetadata({
  params,
}: {
  params: Promise<{ slug?: string[] }>;
}): Promise<Metadata> {
  const { slug } = await params;
  if (!slug?.length) return {};
  const { data } = await loadPage(slug[0]);
  return data ? { title: data.title } : {};
}

export default async function WikiView({
  params,
}: {
  params: Promise<{ slug?: string[] }>;
}) {
  const { slug: seg } = await params;
  const { dict } = await getDict();
  const slug = seg?.[0];
  // U1 模块页守卫（与其余 12+ 条路由同口径）：module_wiki=no 时给统一的
  // 「本站未开放」空态。此前本路由靠 loadIndex 吞掉 4101 再 notFound()
  // ⇒ 成员看到的是「页面不存在」，站长关掉 wiki 却没人知道是关掉了。
  const gate = await requireModule("wiki");
  if (gate) return gate;

  // ============ 目录页：检索框 + 分组卡片 ============
  if (!slug) {
    const { data: items, err } = await loadIndex();
    if (err !== null && isMissing(err)) notFound();
    if (err !== null) return <ErrorPanel error={err} retryHref="/wiki" />;
    if (items === null) notFound();
    if (items.length === 0) {
      return (
        <div className="flex flex-col gap-4">
          <div className={PANEL_BRAND}>
            <h1 className="font-display text-2xl">{dict.wiki.title}</h1>
            <p className="mt-2 text-sm text-sub">{dict.wiki.empty}</p>
          </div>
        </div>
      );
    }
    return (
      <div className="flex flex-col gap-4">
        <div className="flex flex-wrap items-baseline gap-2">
          <h1 className="font-display text-2xl">{dict.wiki.title}</h1>
          <span className="text-sm text-sub">{dict.wiki.subtitle}</span>
        </div>
        <WikiIndex
          entries={items}
          sections={sectionDefs(dict)}
          searchPlaceholder={dict.wiki.searchPlaceholder}
          searchEmpty={dict.wiki.searchEmpty}
          updatedAtLabel={dict.wiki.updatedAt}
          pagesUnit={(n) => dict.wiki.pagesUnit.replace("{n}", String(n))}
        />
      </div>
    );
  }

  // ============ 正文页：三栏（同组导航 / 正文 / TOC） ============
  const [{ data: page, err: pErr }, { data: idx }] = await Promise.all([
    loadPage(slug),
    loadIndex(),
  ]);
  if (pErr !== null && isMissing(pErr)) notFound();
  if (pErr !== null) return <ErrorPanel error={pErr} retryHref="/wiki" />;
  if (!page) notFound();

  const toc = hasBBCode(page.body) ? [] : extractToc(page.body);
  // 左栏：同组页面（当前页高亮）；组内按目录序（sort,id 已在 index 排好）
  const siblings =
    idx?.filter((e) => e.section === page.section && e.slug !== page.slug) ??
    [];
  const w = dict.wiki;
  const sectionTitle =
    sectionDefs(dict).find((s) => s.key === page.section)?.title ?? null;

  return (
    <div className="pting-forums wiki-three">
      {/* 左栏：本组页面导航 */}
      <nav aria-label={w.inThisSection} className="fh-left">
        <div className={PANEL_BRAND}>
          <Link href="/wiki" className="text-xs text-link hover:underline">
            ← {w.backIndex}
          </Link>
          {sectionTitle && (
            <h2 className="mt-2 font-display text-sm font-bold text-ink">
              {sectionTitle}
            </h2>
          )}
          {siblings.length > 0 ? (
            <ul className="mt-2 flex flex-col gap-1">
              {siblings.map((s) => (
                <li key={s.id}>
                  <Link
                    href={`/wiki/${s.slug}`}
                    className={WIKI_SIDE_LINK}
                  >
                    {s.title}
                  </Link>
                </li>
              ))}
            </ul>
          ) : (
            <p className="mt-2 text-xs text-sub">
              {w.updatedAt}: {page.updated_at.slice(0, 10)}
            </p>
          )}
        </div>
      </nav>

      {/* 中栏：标题 + 正文（longform 排版与 /rules /faq 同口径） */}
      <article className={PANEL_BRAND + " min-w-0"}>
        <h1 className="font-display text-2xl">{page.title}</h1>
        <div className="mt-1 flex items-center gap-3 text-xs text-sub">
          <span>
            {w.updatedAt}: {page.updated_at.slice(0, 10)}
          </span>
        </div>
        {/* BBCode → React 节点（XSS 天然免疫）；无 BBCode 标签时回落
         *  forum-markdown 的 markdown-lite（与种子简介同口径） */}
        <div className="longform mt-4 text-sm leading-relaxed">
          {hasBBCode(page.body) ? (
            <div className="whitespace-pre-wrap">
              {renderBBCode(page.body)}
            </div>
          ) : (
            <MarkdownRenderer source={page.body} />
          )}
        </div>
      </article>

      {/* 右栏：本页 TOC（BBCode 正文无标题语法，不显示） */}
      <aside className="fh-side" aria-label={w.tocTitle}>
        {toc.length > 0 && (
          <div className={PANEL_BRAND}>
            <h2 className="font-display text-sm font-bold text-ink">
              {w.tocTitle}
            </h2>
            <ul className="mt-2 flex flex-col gap-1">
              {toc.map((t) => (
                <li key={t.id} className={t.level === 3 ? "pl-3" : ""}>
                  <a
                    href={`#${t.id}`}
                    className={WIKI_TOC_LINK}
                  >
                    {t.text}
                  </a>
                </li>
              ))}
            </ul>
          </div>
        )}
      </aside>
    </div>
  );
}

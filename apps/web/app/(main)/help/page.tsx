import type { Metadata } from "next";
import Link from "next/link";
import { cache } from "react";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";
import { PANEL_BAOZI } from "@/lib/ui-classes";

/** 帮助中心目录（0274）：读 GET /api/v1/help-docs（doc_group 非空的可见页）。
 *  单篇正文复用 /p/{slug}——正文渲染、消毒、深色适配全走那条既有链路，
 *  本页只负责「列出有哪些文档 + 分组」。数据源是 DB，站长在后台
 *  「自定义页面」面板改 slug 前缀 help-* 的那些条目即可（DB 是权威源，
 *  仓库里的 docs/user/*.md 只是导入源，见 scripts/import_user_docs.py）。 */

export const dynamic = "force-dynamic";

interface HelpDoc {
  slug: string;
  title: string;
  group: string;
  sort: number;
}

/** P-313 同族：`/api/v1/help-docs` 失败旧写法返回 []，界面就长成「本站暂无帮助
 *  文档」——取数失败被演成空库，成员与管理组都看不出区别。列表本身没有
 *  「不存在」这一态，故非空失败照抛给路由错误边界。 */
const loadDocs = cache(async (): Promise<HelpDoc[]> =>
  api.get<HelpDoc[]>("/api/v1/help-docs"));

export async function generateMetadata(): Promise<Metadata> {
  const { dict } = await getDict();
  return { title: dict.help.title };
}

export default async function HelpPage() {
  const { dict } = await getDict();
  const docs = await loadDocs();

  // 按 group 聚合，组内保持后端给定的 doc_sort 顺序
  const groups = new Map<string, HelpDoc[]>();
  for (const d of docs) {
    const list = groups.get(d.group) ?? [];
    list.push(d);
    groups.set(d.group, list);
  }

  return (
    <div className="flex flex-col gap-4">
      {/* 页头刻意留手写 .pghd（结构与 PageHeader 原语一致：眉题 / h1 /
          副标题同列堆叠 + 右侧槽）。迁进原语要重算 design_guard 的 h1 台账，
          而那份基线此刻正被另一路改动编辑，不替它记账。 */}
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">{dict.help.eyebrow}</div>
          <h1 className="font-display text-2xl">{dict.help.title}</h1>
          <p className="mt-1 text-xs text-sub">{dict.help.subtitle}</p>
        </div>
        <div className="aside">
          <span className="sticker">
            {fmt(dict.help.docCount, { n: docs.length })}
          </span>
        </div>
      </div>

      {docs.length === 0 ? (
        <div className={PANEL_BAOZI}>
          <p className="text-sm text-sub">{dict.help.empty}</p>
        </div>
      ) : (
        [...groups.entries()].map(([group, list]) => (
          <section key={group} className="baozi-panel">
            <div className="baozi-panel__head">
              {/* 组名用的是文档自己的 doc_group（站长在后台填的那个值），
                  认识的两个码走字典；此前每个分组都渲染同一个
                  `dict.help.groupGuide`，五组点进去长得一模一样，
                  目录等于没分。 */}
              <h2>
                {(dict.help.groups as Record<string, string>)?.[group] ??
                  group}
              </h2>
              <span className="text-xs text-sub">
                {fmt(dict.help.groupCount, { n: list.length })}
              </span>
            </div>
            <ul className="flex flex-col gap-2">
              {list.map((d) => (
                <li key={d.slug}>
                  <Link
                    href={`/p/${d.slug}`}
                    className="flex items-center justify-between gap-3 \
rounded-lg px-3 py-2 hover:bg-soft"
                  >
                    <span className="text-sm">{d.title}</span>
                    <span className="shrink-0 text-xs text-sub">
                      {dict.help.readDoc}
                    </span>
                  </Link>
                </li>
              ))}
            </ul>
          </section>
        ))
      )}

      {/* 「帮助与接口」汇总：样图把这一组（FAQ/站规/封禁公示/API 文档/友链/
          客户端/排行榜）并列在同一组里，站内此前却没有把它们互相连起来——
          其中 /api-docs 尤其离谱：导航、页脚、任何页面都没有一条链接指向它，
          只能靠手敲 URL 到达。这里补一个出口，且不动页头页尾（禁区）。 */}
      <section className="baozi-panel">
        <div className="baozi-panel__head">
          <h2>{dict.help.moreTitle}</h2>
        </div>
        <ul className="flex flex-col divide-y divide-line">
          {[
            { href: "/faq", label: dict.faq.title },
            { href: "/rules", label: dict.rules.title },
            { href: "/ban-log", label: dict.banlog.title },
            { href: "/clients", label: dict.clients.title },
            { href: "/top", label: dict.top.title },
            { href: "/api-docs", label: dict.apidocs.title },
            { href: "/links/apply", label: dict.linkapply.title },
          ].map((l) => (
            <li key={l.href}>
              <Link
                href={l.href}
                className={HUB_ROW}
              >
                <span>{l.label}</span>
                <span aria-hidden="true" className="text-xs text-sub">
                  ›
                </span>
              </Link>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}

const HUB_ROW =
  "flex items-center justify-between gap-3 px-3 py-2 text-sm hover:bg-soft";

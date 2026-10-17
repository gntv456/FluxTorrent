import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { PageHeader } from "@/components/ui/page-header";

export const dynamic = "force-dynamic";

interface RuleItem {
  id: number;
  title: string;
  body: string;
  sort: number;
}

/** 站点规则（rules.php 复刻）：优先展示管理组后台维护的规则（site_rules），
 *  无数据时回落到 i18n 静态规则文案。
 *
 *  2026-10-17 对齐样图 § rules 的四件事：页头换成全站统一的 PageHeader；
 *  正文从「一张单列 nexus-table 里塞满 section」改成一条规则一个面板；
 *  补上样图那张**处罚分级**表（级别 / 措施 / 适用情形——本站没有分级数据表，
 *  内容与站规正文一样走字典，站长要改文案改 i18n，不欠端点）；
 *  结尾给「有疑问去哪」的两个真出口：违规申诉 + 封禁公示名单。
 *
 *  读失败与「后台还没写规则」分开说：旧写法 catch 后直接落静态文案，
 *  界面看不出这是回落——管理组改了规则但接口抖一下，用户看到的还是旧的那份。 */
export default async function RulesPage() {
  const { dict } = await getDict();
  const t = dict.rules;
  let items: RuleItem[] | null = null;
  let failed = false;
  try {
    items = await api.get<RuleItem[]>("/api/v1/rules-content");
  } catch {
    failed = true;
    items = null;
  }

  // id 与 (main)/(auth)/login 三棵路由树的 SkipLink 锚点同口径——顶层
  // 散页不在任何 layout 组里，缺锚则全局 SkipLink 在本页是死链（Lighthouse
  // a11y 的 skip-link 审计实测拦过「No skip link target」）。
  return (
    <main
      id="main-content"
      className="mx-auto w-full max-w-[min(900px,100%)] px-4 py-8"
    >
      <div className="flex flex-col gap-4">
        <PageHeader eyebrow={t.eyebrow} title={t.title} sub={t.sub} />

        {failed && (
          <p className="brand-panel px-4 py-3 text-xs text-sub" role="status">
            {t.loadFailed}
          </p>
        )}

        {(items ?? []).map((r) => (
          <section key={r.id} className="brand-panel">
            <div className="brand-panel__head">
              <h2>{r.title}</h2>
            </div>
            <div className="longform px-4 py-3 text-sm whitespace-pre-wrap leading-relaxed">
              {r.body}
            </div>
          </section>
        ))}

        {(items ?? []).length === 0 && (
          <>
            <p className="text-xs text-sub">{t.updated}</p>
            {t.sections.map((sec) => (
              <section key={sec.head} className="brand-panel">
                <div className="brand-panel__head">
                  <h2>{sec.head}</h2>
                </div>
                <ul className="longform list-disc space-y-1 px-4 py-3 pl-9 text-sm leading-relaxed">
                  {sec.items.map((it) => (
                    <li key={it.slice(0, 24)}>{it}</li>
                  ))}
                </ul>
              </section>
            ))}
          </>
        )}

        <section className="brand-panel overflow-hidden">
          <div className="brand-panel__head">
            <h2>{t.penTitle}</h2>
          </div>
          <div className="overflow-x-auto">
            <table className="nexus-table">
              <tbody>
                <tr>
                  <td className="colhead w-32">{t.penColLevel}</td>
                  <td className="colhead">{t.penColAction}</td>
                  <td className="colhead w-[28%]">{t.penColScope}</td>
                </tr>
                {t.penalties.map((p) => (
                  <tr key={p.level}>
                    <td className="font-bold text-[var(--sky-deep)]">
                      {p.level}
                    </td>
                    <td className="text-sm">{p.action}</td>
                    <td className="text-sm text-sub">{p.scope}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>

        <p className="mt-3 text-xs text-sub">
          {t.appealHint}{" "}
          <Link href="/appeals" className="font-bold text-[var(--sky)] underline">
            {t.appealLink}
          </Link>{" "}
          ·{" "}
          <Link
            href="/ban-log"
            className="font-bold text-[var(--sky)] underline"
          >
            {t.banlogLink}
          </Link>
        </p>
      </div>
    </main>
  );
}

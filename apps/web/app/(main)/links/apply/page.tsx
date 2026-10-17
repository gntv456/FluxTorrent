import { LinkApply } from "@/components/link-apply";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";
import { getPublicLinks } from "@/lib/data";
import { PageHeader } from "@/components/ui/page-header";
import { ReadState } from "@/components/ui/read-state";

export const dynamic = "force-dynamic";

/** 友情链接申请（参考站 linksmanage.php 复刻）。
 *
 *  2026-10-17 对齐样图 § links：页头换 PageHeader 并挂「已收录 N 站」徽章；
 *  正文从「只有一张表单」补成左表单 + 右两张说明卡（收录范围 / 已收录站点）。
 *  右侧那份名单是**新读口** `GET /api/v1/links`：只出 status='active'，
 *  此前站内唯一能看到友链的地方是首页聚合，而那条 SQL 不带状态过滤
 *  （10-17 一并修，见 apps/api/src/http/home.rs）——申请一提交就直接上首页，
 *  等于替站方把没审过的站点挂了出去。 */
export default async function LinkApplyPage() {
  const { dict } = await getDict();
  const t = dict.linkapply;
  const links = await getPublicLinks();

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow={t.eyebrow}
        title={t.title}
        sub={t.subtitle}
        aside={
          links === null ? undefined : (
            <span className="sticker">
              {fmt(t.headCount, { n: links.length })}
            </span>
          )
        }
      />

      <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_300px]">
        <LinkApply />

        <aside className="flex flex-col gap-4">
          <section className="baozi-panel">
            <div className="baozi-panel__head">
              <h2>{t.scopeTitle}</h2>
            </div>
            <p className="px-4 py-3 text-xs leading-relaxed text-sub">
              {t.scopeBody}
            </p>
          </section>

          <section className="baozi-panel overflow-hidden">
            <div className="baozi-panel__head">
              <h2>{t.listTitle}</h2>
            </div>
            <ReadState
              failed={links === null}
              empty={(links ?? []).length === 0}
              dict={dict}
              emptyTitle={t.listEmpty}
              emptyDesc={t.listEmptyHint}
              emptyIcon="link"
            >
              <ul className="divide-y divide-line">
                {(links ?? []).map((l) => (
                  <li key={l.url} className="px-4 py-2">
                    <a
                      href={l.url}
                      target="_blank"
                      rel="nofollow noreferrer"
                      className={LINK_CLS}
                      title={l.title ?? l.name}
                    >
                      {l.name}
                    </a>
                    {l.title && (
                      <span className="block truncate text-xs text-sub">
                        {l.title}
                      </span>
                    )}
                  </li>
                ))}
              </ul>
            </ReadState>
          </section>
        </aside>
      </div>
    </div>
  );
}

const LINK_CLS =
  "block truncate text-sm font-bold underline text-[var(--sky)]";

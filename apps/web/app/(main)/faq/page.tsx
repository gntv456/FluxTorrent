import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { PageHeader } from "@/components/ui/page-header";
import { ReadState } from "@/components/ui/read-state";
import { fmt } from "@/i18n/config";

export const dynamic = "force-dynamic";

interface FaqItem {
  id: number;
  category: string;
  question: string;
  answer: string;
  sort: number;
}

/** FAQ（faq.php 口径）：按分类分组的问答列表。
 *  2026-10-17 两处收口：① 取档失败不再和「站长还没写 FAQ」共用一句话
 *  （旧写法 `catch {}` 后 items=[] 直接落进空态，接口抖一下页面就宣布
 *  「暂无 FAQ 条目」，同批五那族账）；② 页头补副标题与条目数，与样图 § faq 对齐。 */
export default async function FaqPage() {
  const { dict } = await getDict();
  const t = dict.faq;
  let items: FaqItem[] | null = null;
  try {
    items = await api.get<FaqItem[]>("/api/v1/faq");
  } catch {
    items = null;
  }
  const list = items ?? [];
  const groups = new Map<string, FaqItem[]>();
  for (const it of list) {
    const arr = groups.get(it.category) ?? [];
    arr.push(it);
    groups.set(it.category, arr);
  }
  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow={t.eyebrow}
        title={t.title}
        sub={t.sub}
        aside={
          items === null ? undefined : (
            <span className="sticker">
              {fmt(t.itemCount, { n: list.length, g: groups.size })}
            </span>
          )
        }
      />
      <ReadState
        failed={items === null}
        empty={list.length === 0}
        dict={dict}
        emptyTitle={t.empty}
        emptyDesc={t.emptyHint}
        emptyIcon="doc"
      >
        {[...groups.entries()].map(([cat, rows]) => (
          <section key={cat} className="baozi-panel">
            <div className="baozi-panel__head">
              <h2>{cat === "default" ? t.defaultCat : cat}</h2>
              <span className="text-xs text-sub">
                {fmt(t.groupCount, { n: rows.length })}
              </span>
            </div>
            {/* M6.3：FAQ 问答转 details 折叠（概念稿 §15.4）——
             *  桌面也用折叠（答案默认收起、点开阅读），窄屏长答案
             *  不再撑出 rowhead 38% + rowfollow 的双列横滚 */}
            <div className="faq-list">
              {rows.map((f) => (
                <details key={f.id} className="faq-item">
                  <summary>{f.question}</summary>
                  <p className="longform">{f.answer}</p>
                </details>
              ))}
            </div>
          </section>
        ))}
      </ReadState>
    </div>
  );
}

import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

interface FaqItem {
  id: number;
  category: string;
  question: string;
  answer: string;
  sort: number;
}

/** FAQ（faq.php 口径）：按分类分组的问答列表 */
export default async function FaqPage() {
  const { dict } = await getDict();
  let items: FaqItem[] = [];
  try {
    items = await api.get<FaqItem[]>("/api/v1/faq");
  } catch {
    // 未登录/后端不可达 → 空列表
  }
  const groups = new Map<string, FaqItem[]>();
  for (const it of items) {
    const list = groups.get(it.category) ?? [];
    list.push(it);
    groups.set(it.category, list);
  }
  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">FAQ</div>
          <h1 className="font-display text-2xl">{dict.faq.title}</h1>
        </div>
      </div>
      {[...groups.entries()].map(([cat, list]) => (
        <section key={cat} className="baozi-panel">
          <div className="baozi-panel__head">
            <h2>{cat === "default" ? dict.faq.defaultCat : cat}</h2>
          </div>
          {/* M6.3：FAQ 问答转 details 折叠（概念稿 §15.4）——
           *  桌面也用折叠（答案默认收起、点开阅读），窄屏长答案
           *  不再撑出 rowhead 38% + rowfollow 的双列横滚 */}
          <div className="faq-list">
            {list.map((f) => (
              <details key={f.id} className="faq-item">
                <summary>{f.question}</summary>
                <p className="longform">{f.answer}</p>
              </details>
            ))}
          </div>
        </section>
      ))}
      {items.length === 0 && (
        <p className="baozi-panel p-4 text-sm text-sub">{dict.faq.empty}</p>
      )}
    </div>
  );
}

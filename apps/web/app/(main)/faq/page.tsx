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
      <h1 className="font-display text-2xl">{dict.faq.title}</h1>
      {[...groups.entries()].map(([cat, list]) => (
        <table key={cat} className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead" colSpan={2}>
                <h2 className="font-display">
                  {cat === "default" ? dict.faq.defaultCat : cat}
                </h2>
              </td>
            </tr>
            {list.map((f) => (
              <tr key={f.id}>
                <td className="rowhead w-[38%] align-top">{f.question}</td>
                <td className="rowfollow">{f.answer}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ))}
      {items.length === 0 && (
        <p className="baozi-panel p-4 text-sm text-sub">{dict.faq.empty}</p>
      )}
    </div>
  );
}

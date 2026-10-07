import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

interface RuleItem {
  id: number;
  title: string;
  body: string;
  sort: number;
}

/** 站点规则（rules.php 复刻）：优先展示管理组后台维护的规则（site_rules），
 *  无数据时回落到 i18n 静态规则文案 */
export default async function RulesPage() {
  const { dict } = await getDict();
  let items: RuleItem[] = [];
  try {
    items = await api.get<RuleItem[]>("/api/v1/rules-content");
  } catch {
    // 未登录/后端不可达 → 回落静态文案
  }
  return (
    <main className="mx-auto w-full max-w-[min(900px,100%)] px-4 py-8">
      <p className="mb-3 text-xs text-sub">
        <Link href="/login" className="text-sky">
          FluxTorrent
        </Link>{" "}
        » {dict.rules.title}
      </p>
      {items.length > 0 ? (
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead">{dict.rules.title}</td>
            </tr>
          </thead>
          <tbody>
            {items.map((r) => (
              <tr key={r.id}>
                <td className="p-4">
                  <section className="longform">
                    <h2 className="font-display text-base font-bold text-[var(--baozi-orange-dark)]">
                      {r.title}
                    </h2>
                    <p className="mt-2 whitespace-pre-wrap leading-relaxed">
                      {r.body}
                    </p>
                  </section>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead">{dict.rules.title}</td>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td className="p-4">
                <p className="text-[12px] text-sub">{dict.rules.updated}</p>
                {dict.rules.sections.map((sec) => (
                  <section key={sec.head} className="longform mt-4">
                    <h2 className="font-display text-base font-bold text-[var(--baozi-orange-dark)]">
                      {sec.head}
                    </h2>
                    <ul className="mt-2 list-disc space-y-1 pl-5 leading-relaxed">
                      {sec.items.map((it) => (
                        <li key={it.slice(0, 24)}>{it}</li>
                      ))}
                    </ul>
                  </section>
                ))}
              </td>
            </tr>
          </tbody>
        </table>
      )}
    </main>
  );
}

import Link from "next/link";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** 站点规则（NexusPHP rules.php 复刻：colhead 标题条 + 分节盒子） */
export default async function RulesPage() {
  const { dict } = await getDict();
  return (
    <main className="mx-auto w-full max-w-[900px] px-4 py-8">
      <p className="mb-3 text-xs text-sub">
        <Link href="/login" className="text-sky">
          FluxTorrent
        </Link>{" "}
        » {dict.rules.title}
      </p>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">{dict.rules.title}</td>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td className="p-4">
              <p className="text-[11px] text-sub">{dict.rules.updated}</p>
              {dict.rules.sections.map((sec) => (
                <section key={sec.head} className="mt-4">
                  <h2 className="font-display text-base font-bold text-[var(--baozi-orange-dark)]">
                    {sec.head}
                  </h2>
                  <ul className="mt-2 list-disc space-y-1 pl-5 text-sm leading-relaxed">
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
    </main>
  );
}

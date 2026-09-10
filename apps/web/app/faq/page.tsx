import Link from "next/link";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** 常见问题（NexusPHP FAQ.php 复刻：colhead 标题条 + 问答分节盒子） */
export default async function FaqPage() {
  const { dict } = await getDict();
  return (
    <main className="mx-auto w-full max-w-[900px] px-4 py-8">
      <p className="mb-3 text-xs text-sub">
        <Link href="/login" className="text-sky">
          FluxTorrent
        </Link>{" "}
        » {dict.faqPage.title}
      </p>
      <div className="flex flex-col gap-4">
        {dict.faqPage.items.map((it, i) => (
          <table key={it.q.slice(0, 24)} className="nexus-table">
            <thead>
              <tr>
                <td className="colhead">
                  {i + 1}. {it.q}
                </td>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td className="p-4 text-sm leading-relaxed">{it.a}</td>
              </tr>
            </tbody>
          </table>
        ))}
        <p className="text-center text-xs text-sub">{dict.faqPage.updated}</p>
      </div>
    </main>
  );
}

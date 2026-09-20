import Link from "next/link";
import { getDict } from "@/i18n/server";

/** 离线回退页（M26：SW 导航失败时的兜底） */
export default async function OfflinePage() {
  const { dict } = await getDict();
  return (
    <div className="mx-auto flex max-w-sm flex-col items-center gap-4 py-16 text-center">
      <span aria-hidden className="text-[72px] leading-none">
        🦉💤
      </span>
      <h1 className="font-display text-2xl">{dict.offline.title}</h1>
      <p className="text-sm text-sub">{dict.offline.subtitle}</p>
      <Link
        href="/"
        className="min-h-[44px] inline-flex items-center rounded-full bg-sky px-6 font-bold text-white"
      >
        {dict.offline.home}
      </Link>
    </div>
  );
}

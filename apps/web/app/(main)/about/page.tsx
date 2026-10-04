import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { getSiteProfile } from "@/lib/site-profile";

export const dynamic = "force-dynamic";

interface AboutInfo {
  product: string;
  version: string;
  site_name: string;
  source_url: string;
  docs_url: string;
}

const CARD =
  "rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-3";
const FOOT_NOTE =
  "border-t border-dashed border-line pt-3 text-[11px] text-sub";

/** 版本页（aboutnexus.php 口径）：页尾「Powered by FluxTorrent」的落点。
 *  展示本站运行的程序版本 + 开源地址 + 文档——站长与用户都能据此报障。 */
export default async function AboutPage() {
  const { dict } = await getDict();
  const t = dict.about;
  const info = await api.get<AboutInfo>("/api/v1/about").catch(() => null);
  const profile = await getSiteProfile();
  const year = new Date().getFullYear();

  return (
    <main className="mx-auto w-full max-w-[min(900px,100%)] px-4 py-8">
      <p className="mb-3 text-xs text-sub">
        <Link href="/" className="text-sky">
          {profile.brand || "FluxTorrent"}
        </Link>{" "}
        » {t.title}
      </p>

      <div className="baozi-panel flex flex-col gap-4 p-5">
        <h1 className="font-display text-lg font-bold">
          {t.title}
        </h1>

        <section className="longform text-sm leading-relaxed">
          <p>{t.intro}</p>
        </section>

        <dl className="grid gap-2 text-sm md:grid-cols-2">
          <div className={CARD}>
            <dt className="text-xs text-sub">{t.version}</dt>
            <dd className="mt-1 font-bold">
              {info ? `${info.product} ${info.version}` : "—"}
            </dd>
          </div>
          <div className={CARD}>
            <dt className="text-xs text-sub">{t.siteTitle}</dt>
            <dd className="mt-1 font-bold">
              {info?.site_name || profile.brand}
            </dd>
          </div>
        </dl>

        <section className="flex flex-wrap gap-x-5 gap-y-2 text-sm">
          <a
            className="text-link"
            href={info?.source_url || "https://github.com/gntv456/FluxTorrent"}
            target="_blank"
            rel="noreferrer"
          >
            {t.source}
          </a>
          <a
            className="text-link"
            href={info?.docs_url || "https://wiki.ptang.top/ft/"}
            target="_blank"
            rel="noreferrer"
          >
            {t.docs}
          </a>
        </section>

        <p className={FOOT_NOTE}>
          © 2024–{year} FluxTorrent · Rust + Next.js
        </p>
      </div>
    </main>
  );
}

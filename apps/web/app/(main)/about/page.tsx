import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { getSiteProfile } from "@/lib/site-profile";
import { PageHeader } from "@/components/ui/page-header";

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
const ROW = "flex items-baseline justify-between gap-3 px-4 py-2";
const FOOT_NOTE =
  "border-t border-dashed border-line pt-3 text-[11px] text-sub";

/** 版本页（aboutnexus.php 口径）：页尾「Powered by FluxTorrent」的落点。
 *  展示本站运行的程序版本 + 站点信息 + 开源地址与文档——站长与用户都能据此报障。
 *
 *  2026-10-17 对齐样图 § about：原先两张卡（版本/站名）补成三张——
 *  「站组类型」取 site-profile 的 pack_name（站型包名，后台改站型就会跟着变），
 *  再加右侧的站点信息一览（版本 / 站组 / 建站时间）。取档失败不再默默显示「—」，
 *  多一句说明。 */
export default async function AboutPage() {
  const { dict } = await getDict();
  const t = dict.about;
  const info = await api.get<AboutInfo>("/api/v1/about").catch(() => null);
  const profile = await getSiteProfile();
  const year = new Date().getFullYear();

  return (
    <main className="mx-auto w-full max-w-[min(900px,100%)] px-4 py-8">
      <div className="flex flex-col gap-4">
        <PageHeader
          eyebrow={t.eyebrow}
          title={t.title}
          sub={t.subtitle}
          aside={
            info ? (
              <span className="sticker">
                {info.product} v{info.version}
              </span>
            ) : undefined
          }
        />

        <section className="longform text-sm leading-relaxed">
          <p>{t.intro}</p>
        </section>

        {!info && (
          <p className="brand-panel px-4 py-3 text-xs text-sub" role="status">
            {t.loadFailed}
          </p>
        )}

        <dl className="grid gap-2 text-sm md:grid-cols-3">
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
          <div className={CARD}>
            <dt className="text-xs text-sub">{t.typeTitle}</dt>
            <dd className="mt-1 font-bold">
              {/* pack_name 是站型包的显示名（如「综合站」）；没配站型包时
                  回落到站型码，不留白 */}
              {profile.pack_name || profile.site_type || "—"}
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
            href={
              info?.docs_url ||
              "https://github.com/gntv456/FluxTorrent#readme"
            }
            target="_blank"
            rel="noreferrer"
          >
            {t.docs}
          </a>
        </section>

        <section className="brand-panel">
          <div className="brand-panel__head">
            <h2>{t.asideTitle}</h2>
          </div>
          <ul className="divide-y divide-line text-sm">
            <li className={ROW}>
              <span className="text-xs text-sub">{t.version}</span>
              <span className="num font-bold">
                {info ? `v${info.version}` : "—"}
              </span>
            </li>
            <li className={ROW}>
              <span className="text-xs text-sub">{t.typeTitle}</span>
              <span className="font-bold">
                {profile.pack_name || profile.site_type || "—"}
              </span>
            </li>
            {profile.founded && (
              <li className={ROW}>
                <span className="text-xs text-sub">{t.founded}</span>
                <span className="num font-bold">{profile.founded}</span>
              </li>
            )}
            <li className={ROW}>
              <span className="text-xs text-sub">{t.selfLink}</span>
              <Link
                href="/top"
                className="font-bold underline text-[var(--sky)]"
              >
                {dict.nav.top}
              </Link>
            </li>
          </ul>
        </section>

        <p className={FOOT_NOTE}>
          © 2024–{year} {info?.product || "FluxTorrent"} ·{" "}
          {profile.brand || "FluxTorrent"}
        </p>
      </div>
    </main>
  );
}

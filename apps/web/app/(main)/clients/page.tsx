import { getSiteProfile } from "@/lib/site-profile";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/**
 * BT 客户端指南（0283 P1-7）：新手第一坑是 DHT/PEX 不关（私站口径下等于
 * 泄漏汇报口径还可能触发作弊判定）。本页给推荐客户端列表 + 关键配置 +
 * 私站纪律。内容全走 i18n（clients.items），站点品牌名运行时注入。
 */

export default async function ClientsPage() {
  const { dict } = await getDict();
  const profile = await getSiteProfile().catch(() => null);
  const brand = profile?.brand || dict.common.brand;
  const t = dict.clients;

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{t.title}</h1>
      <p className="text-sm text-muted">{t.intro.replace("{brand}", brand)}</p>

      <section className="flex flex-col gap-3">
        {t.items.map((c) => (
          <div
            key={c.name}
            className={`client-card${c.banned ? " client-card--banned" : ""}`}
          >
            <header className="client-card__head">
              <h2 className="client-card__name">
                {c.name}
                {c.rec && (
                  <span className="client-card__rec">{t.recommended}</span>
                )}
                {c.banned && (
                  <span className="client-card__ban">{t.banned}</span>
                )}
              </h2>
              <span className="client-card__ver">
                {c.ver} · {c.os}
              </span>
            </header>
            <ul className="client-card__keys">
              {c.keys.map((k) => (
                <li key={k}>{k}</li>
              ))}
            </ul>
          </div>
        ))}
      </section>

      <section className="client-rules">
        <h2 className="font-semibold">{t.rulesTitle}</h2>
        <ul className="list-disc pl-5 text-sm">
          {t.rules.map((r) => (
            <li key={r}>{r}</li>
          ))}
        </ul>
      </section>
    </div>
  );
}

import Link from "next/link";
import { cookies } from "next/headers";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { accountHealthCopy } from "@/i18n/account-health";
import { PageHeader } from "@/components/ui/page-header";
import { ReadState } from "@/components/ui/read-state";
import { EmptyState } from "@/components/ui/empty-state";
import { BTN_MD_SKY } from "@/lib/ui-classes";
import { RiskCard, type MyRisk } from "./_parts/risk-card";
import {
  OnboardingCard,
  type Onboarding,
} from "./_parts/onboarding-card";
import { PolicyCard, type SitePolicy } from "./_parts/policy-card";

export const dynamic = "force-dynamic";

/** 账户健康（会员侧）：把「为什么我被判定」「我还能补什么」与
 *  「站点生效的规则」放在同一页。
 *
 *  三个取数各自兜底：单口失败只降级那一块，不把整页演成读取失败；
 *  未登录（无 flux_token）直接给登录引导，而不是去后端换 401。 */
export default async function AccountHealthPage() {
  const { dict, locale } = await getDict();
  const t = accountHealthCopy[locale];
  const store = await cookies();
  const signedIn = !!store.get("flux_token")?.value;
  if (!signedIn) {
    return (
      <div className="flex flex-col gap-4">
        <PageHeader
          eyebrow={dict.my.title}
          title={t.title}
          sub={t.sub}
        />
        <EmptyState
          icon="lock"
          tone="warn"
          title={t.loginRequired}
          desc={t.loginHint}
          action={
            <Link
              href="/login?next=%2Faccount-health"
              className={`inline-flex items-center ${BTN_MD_SKY}`}
            >
              {t.loginCta}
            </Link>
          }
        />
      </div>
    );
  }
  const [risk, onboarding, policy] = await Promise.all([
    api.get<MyRisk>("/api/v1/me/risk").catch(() => null),
    api.get<Onboarding>("/api/v1/me/onboarding").catch(() => null),
    api.get<SitePolicy>("/api/v1/system/policies").catch(() => null),
  ]);
  const failed =
    risk === null && onboarding === null && policy === null;
  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow={dict.my.title}
        title={t.title}
        sub={t.sub}
        aside={
          <Link href="/my" className="text-link text-sm">
            {t.backToCenter}
          </Link>
        }
      />
      <ReadState failed={failed} dict={dict} emptyTitle={t.title}>
        <div
          className="grid gap-4 lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]"
        >
          <div className="flex min-w-0 flex-col gap-4">
            {risk ? (
              <RiskCard t={t} locale={locale} risk={risk} />
            ) : (
              <EmptyState
                icon="info"
                tone="warn"
                title={dict.readState.failTitle}
                desc={dict.readState.failDesc}
              />
            )}
            {onboarding ? (
              <OnboardingCard t={t} data={onboarding} />
            ) : (
              <EmptyState
                compact
                icon="info"
                tone="warn"
                title={dict.readState.failTitle}
                desc={dict.readState.failDesc}
              />
            )}
          </div>
          <div className="flex min-w-0 flex-col gap-4">
            {policy ? (
              <PolicyCard t={t} locale={locale} policy={policy} />
            ) : (
              <EmptyState
                compact
                icon="info"
                tone="warn"
                title={dict.readState.failTitle}
                desc={dict.readState.failDesc}
              />
            )}
          </div>
        </div>
      </ReadState>
    </div>
  );
}

import Link from "next/link";
import type { AccountHealthCopy } from "@/i18n/account-health";
import { fmt } from "@/i18n/config";
import { Badge } from "@/components/ui/badge";
import { Icon } from "@/components/icons";

export interface OnboardingStep {
  key: string;
  done: boolean;
  href: string;
}

export interface Onboarding {
  steps: OnboardingStep[];
  done: number;
  total: number;
  completed: boolean;
}

/** 新手清单卡：每一步的 done 都是后端按库内事实算的，这里只负责展示。
 *  未完成的步骤直接给可点的去处，避免「知道缺什么却不知道去哪补」。 */
export function OnboardingCard({
  t,
  data,
}: {
  t: AccountHealthCopy;
  data: Onboarding;
}) {
  const pct =
    data.total > 0 ? Math.round((data.done / data.total) * 100) : 0;
  return (
    <section className="brand-panel">
      <div className="brand-panel__head">
        <h2>{t.onboardTitle}</h2>
        <span className="text-xs font-semibold text-sub">
          {data.completed
            ? t.onboardAllDone
            : fmt(t.onboardProgress, {
                done: data.done,
                total: data.total,
              })}
        </span>
      </div>
      <div className="flex flex-col gap-3 p-4">
        <p className="-mt-1 text-xs text-sub">{t.onboardSub}</p>
        <div
          className="h-2 overflow-hidden rounded-full
            bg-[var(--border-soft)]"
          role="progressbar"
          aria-label={t.onboardTitle}
          aria-valuenow={data.done}
          aria-valuemin={0}
          aria-valuemax={data.total}
        >
          <div
            className="h-full bg-[var(--accent)]"
            style={{ width: `${pct}%` }}
          />
        </div>
        <ol className="flex flex-col divide-y divide-line">
          {data.steps.map((s) => {
            const c = t.steps[s.key];
            return (
              <li key={s.key} className="flex items-start gap-3 py-2">
                <span
                  className={
                    s.done
                      ? "mt-0.5 text-[var(--tone-success)]"
                      : "mt-0.5 text-sub"
                  }
                >
                  <Icon
                    name={s.done ? "checkCircle" : "clock"}
                    size={16}
                  />
                </span>
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-bold">
                    {c?.label ?? s.key}
                  </p>
                  <p className="text-xs text-sub">{c?.desc ?? ""}</p>
                </div>
                {s.done ? (
                  <Badge tone="success" size="sm">
                    {t.stepDone}
                  </Badge>
                ) : (
                  <Link
                    href={s.href}
                    className="shrink-0 text-link text-xs"
                  >
                    {t.stepTodo}
                  </Link>
                )}
              </li>
            );
          })}
        </ol>
        {/* 0499 新手旅程：清单完成的「然后呢」——从生存（装客户端/首下/
            首做种）接到贡献（契约/保种/考核）。此前完成态只有一枚徽章，
            新人第一周后断链。 */}
        {data.completed && (
          <div className="rounded border border-[var(--line)] p-3">
            <p className="mb-2 text-sm font-bold">{t.nextTitle}</p>
            <div className="flex flex-wrap gap-3 text-xs">
              <Link href="/contracts" className="text-link">
                {t.nextContracts}
              </Link>
              <Link href="/preserve" className="text-link">
                {t.nextPreserve}
              </Link>
              <Link href="/me/exams" className="text-link">
                {t.nextExams}
              </Link>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

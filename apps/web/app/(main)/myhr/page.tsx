import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";
import { accountHealthCopy } from "@/i18n/account-health";
import { MyHrTable } from "@/components/my-hr";
import Link from "next/link";
import { PageHeader } from "@/components/ui/page-header";
import { ReadState } from "@/components/ui/read-state";
import { Pager } from "@/components/ui/pager";
import {
  HR_STATUSES,
  HrKpiStrip,
  HrStatusFilter,
} from "./_parts/hr-tools";

export const dynamic = "force-dynamic";

const PER = 20;

interface HrRow {
  torrent_id: number;
  torrent_name: string;
  required_seconds: number;
  seeded_seconds: number;
  deadline: string;
  status: string;
  /** 真实下载完成时刻（snatches.completed_at），脏数据可为 null */
  downloaded_at?: string | null;
}

interface MyHrResp {
  rows: HrRow[];
  total: number;
  page: number;
  per: number;
  /** 后端回显的生效筛选（白名单结果），null = 不筛 */
  status: string | null;
  counts: {
    open: number;
    satisfied: number;
    violated: number;
    pardoned: number;
  };
  default_seed_hours: number;
  default_days: number;
  /** 生效策略的版本与更新时间（与 /system/policies 同源） */
  policy_version: string;
  policy_updated_at: string | null;
}

const ZERO_COUNTS = {
  open: 0,
  satisfied: 0,
  violated: 0,
  pardoned: 0,
};

/** 我的 H&R（myhr.php 复刻）：完成下载的种子做种时长与达标状态。
 *  H&R 执法（hr_enforce）独立于 exams 模块在跑，用户记录页不再挂
 *  exams 守卫——否则 exams 关闭时形成「执法在跑、记录不可见」断链。
 *  规则文案插值（深测 2026-10-03）：hours/days 来自后端站点设定
 *  （hr_hours 键，与 worker 建快照同源），不再写死 120h。
 *
 *  本轮对齐样图 § myhr：分页与状态筛选改由 URL 驱动（此前一次性拉
 *  `LIMIT 100` 到底，记录多了就翻不动也筛不了）；补 KPI 条、下载时间
 *  列与「剩 N 天 / 已逾期 N 天」；取档失败不再演成「暂无记录」。 */
export default async function MyHrPage({
  searchParams,
}: {
  searchParams: Promise<{ page?: string; status?: string }>;
}) {
  const sp = await searchParams;
  const { dict, locale } = await getDict();
  const t = dict.myhr;
  const ah = accountHealthCopy[locale];
  const status = (HR_STATUSES as readonly string[]).includes(sp.status ?? "")
    ? (sp.status as string)
    : "";
  const pageNum = Number(sp.page);
  const page =
    Number.isFinite(pageNum) && pageNum > 1 ? Math.floor(pageNum) : 1;

  let data: MyHrResp | null = null;
  try {
    const q = new URLSearchParams({ page: String(page), per: String(PER) });
    if (status) q.set("status", status);
    data = await api.get<MyHrResp>(`/api/v1/me/hr?${q.toString()}`);
  } catch {
    // 未登录 / 后端不可达：留 null 交给 ReadState 说「读取失败」
    data = null;
  }

  // 「全部」档不写 status，page=1 不写 page：URL 只带偏离默认的那几段
  const href = (s: string, p: number) => {
    const q = new URLSearchParams();
    if (s) q.set("status", s);
    if (p > 1) q.set("page", String(p));
    const qs = q.toString();
    return qs ? `/myhr?${qs}` : "/myhr";
  };
  const rows = data?.rows ?? [];
  const per = data?.per ?? PER;
  const total = data?.total ?? 0;
  const pages = Math.max(1, Math.ceil(total / per));
  const hrefs = Array.from({ length: pages }, (_, i) =>
    href(status, i + 1),
  );
  const hours = data?.default_seed_hours ?? 48;
  const days = data?.default_days ?? 14;

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow={`${dict.my.title} / ${t.title}`}
        title={t.title}
        aside={
          data ? (
            <span className="sticker">
              {t.kpiTotal} {total.toLocaleString()}
            </span>
          ) : undefined
        }
      />

      {data && (
        <p
          className={
            "rounded-[var(--r-md)] p-3 text-sm text-ink " +
            "bg-[var(--sky-soft)]"
          }
        >
          {fmt(t.rule, { days, hours })}
        </p>
      )}

      {data?.policy_version && (
        <p className="-mt-2 text-xs text-sub">
          {fmt(ah.policyVersion, { version: data.policy_version })}
          {" · "}
          <Link href="/account-health" className="text-link">
            {ah.title}
          </Link>
        </p>
      )}

      {data && (
        <HrKpiStrip
          t={t}
          total={total}
          counts={data.counts ?? ZERO_COUNTS}
        />
      )}

      <HrStatusFilter
        t={t}
        status={status}
        hrefFor={(v) => href(v, 1)}
      />

      <ReadState
        failed={data === null}
        dict={dict}
        emptyTitle={t.empty}
      >
        <div className="brand-panel">
          <div className="brand-panel__head">
            <h2>{t.panelTitle}</h2>
          </div>
          <MyHrTable
            rows={rows}
            locale={locale}
            status={status}
            now={Date.now()}
          />
        </div>
      </ReadState>

      {data && pages > 1 && (
        <Pager page={page} pages={pages} total={total} hrefs={hrefs} />
      )}
    </div>
  );
}

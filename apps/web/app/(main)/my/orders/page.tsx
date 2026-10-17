import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { requireModule } from "@/components/module-gate";
import {
  MyOrdersTable,
  type FulfillKey,
  type OrdersEnvelope,
} from "@/components/my-orders-table";
import { PageHeader } from "@/components/ui/page-header";
import { FilterLinks } from "@/components/ui/filter-links";
import { Pager } from "@/components/ui/pager";
import { ReadState } from "@/components/ui/read-state";

export const dynamic = "force-dynamic";

/** 我的订单（商城审计 P2-1 + 0383 履约态）：shop_orders 用户侧第一张表。
 *  之前订单只写不读——买了什么只能去流水里看一排 'shop'。
 *
 *  样图 § my-orders 对齐：状态四档筛选（全部/处理中/已完成/异常退款）
 *  走 `?fulfill=` 链接档、翻页走 `?page=` 链接档，一个客户端 state
 *  都没有——禁用 JS 也能筛能翻。 */
export default async function MyOrdersPage({
  searchParams,
}: {
  searchParams: Promise<{ page?: string; fulfill?: string }>;
}) {
  const gate = await requireModule("shop");
  if (gate) return gate;

  const sp = await searchParams;
  const { dict, currency } = await getDict();
  const t = dict.myOrders;
  const fulfill: FulfillKey =
    sp.fulfill === "pending" ||
    sp.fulfill === "done" ||
    sp.fulfill === "failed"
      ? sp.fulfill
      : "";
  const page = Math.min(10_000, Math.max(1, Number(sp.page) || 1));
  const qs = (p: number, f: FulfillKey) => {
    const q = new URLSearchParams();
    if (f) q.set("fulfill", f);
    if (p > 1) q.set("page", String(p));
    const s = q.toString();
    return s ? `/my/orders?${s}` : "/my/orders";
  };
  // 后端 page 有上界（clamp 1..10000），前端跟着夹，避免翻到不存在的页
  const query = new URLSearchParams({ page: String(page) });
  if (fulfill) query.set("fulfill", fulfill);
  const data = await api
    .get<OrdersEnvelope>(`/api/v1/me/orders?${query.toString()}`)
    .catch(() => null);
  const per = data?.per_page ?? 20;
  const pages = Math.max(1, Math.ceil((data?.total ?? 0) / per));

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow={`${dict.my.title} / ${t.title}`}
        title={t.title}
        sub={t.subtitle}
      />
      <FilterLinks
        label={t.colStatus}
        value={fulfill}
        options={[
          { label: t.tabAll, href: qs(1, "") },
          { label: t.tabPending, value: "pending", href: qs(1, "pending") },
          { label: t.tabDone, value: "done", href: qs(1, "done") },
          { label: t.tabFailed, value: "failed", href: qs(1, "failed") },
        ]}
      />
      <ReadState
        failed={data === null}
        dict={dict}
        emptyTitle={t.empty}
      >
        <MyOrdersTable
          rows={data?.rows ?? []}
          total={data?.total ?? 0}
          fulfill={fulfill}
          dict={t}
          currency={currency}
        />
      </ReadState>
      {data && pages > 1 && (
        <Pager
          page={data.page}
          pages={pages}
          total={data.total}
          hrefs={Array.from({ length: pages }, (_, i) =>
            qs(i + 1, fulfill),
          )}
        />
      )}
    </div>
  );
}

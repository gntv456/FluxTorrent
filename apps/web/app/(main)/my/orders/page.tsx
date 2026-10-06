import { getMyOrders } from "@/lib/data";
import { getDict } from "@/i18n/server";
import { requireModule } from "@/components/module-gate";
import { MyOrdersTable } from "@/components/my-orders-table";

export const dynamic = "force-dynamic";

/** 我的订单（商城审计 P2-1）：shop_orders 用户侧第一张表。
 *  之前订单只写不读——买了什么只能去流水里看一排 'shop'。 */
export default async function MyOrdersPage({
  searchParams,
}: {
  searchParams: Promise<{ page?: string }>;
}) {
  const gate = await requireModule("shop");
  if (gate) return gate;

  const sp = await searchParams;
  const pageNum = Math.max(1, Number(sp.page) || 1);
  const { dict, currency } = await getDict();
  const data = await getMyOrders(pageNum);
  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">Orders</div>
          <h1 className="font-display text-2xl">{dict.myOrders.title}</h1>
        </div>
        <span className="sub">{dict.myOrders.subtitle}</span>
      </div>
      <MyOrdersTable
        rows={data?.rows ?? []}
        total={data?.total ?? 0}
        page={data?.page ?? 1}
        perPage={data?.per_page ?? 20}
        dict={dict.myOrders}
        currency={currency}
      />
    </div>
  );
}

import { getShopItems } from "@/lib/data";
import { BuyButton } from "@/components/buy-button";

export const dynamic = "force-dynamic";

export default async function ShopPage() {
  const items = await getShopItems();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">火花商店</h1>
        <span className="text-sm text-sub">用做种收获的火花兑换好物</span>
      </div>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {items.map((item) => (
          <div
            key={item.id}
            className="flex flex-col gap-2 rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)]"
          >
            <div className="flex items-center justify-between">
              <h2 className="font-bold">{item.name}</h2>
              <span className="sticker bg-sun text-ink num">
                {item.price.toLocaleString("zh-CN")}
              </span>
            </div>
            <p className="text-xs text-sub">
              {item.kind === "upload_credit" ? "上传量提升" : "站点权益"}
            </p>
            <BuyButton itemId={item.id} name={item.name} price={item.price} />
          </div>
        ))}
      </div>
    </div>
  );
}

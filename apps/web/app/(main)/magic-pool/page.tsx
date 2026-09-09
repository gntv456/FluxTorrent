import { getPool } from "@/lib/data";

export const dynamic = "force-dynamic";

export default async function MagicPoolPage() {
  const pool = await getPool();
  const pct = pool ? Math.min(100, Math.round(pool.progress * 100)) : 0;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">站免池</h1>
        <span className="text-sm text-sub">全站合力，下月双免</span>
      </div>

      <section className="rounded-[var(--r-lg)] border border-line bg-white p-6 shadow-[var(--shadow-card)]">
        <p className="text-sm text-sub">
          当月累计捐赠达 <b className="text-ink">200 万</b> 火花，下月 1–3 号自动开启全站双倍免费促销。
        </p>
        {/* 彩带进度条（§3.2 品牌元素） */}
        <div className="mt-4 h-3 w-full overflow-hidden rounded-full bg-cloud">
          <div
            className="h-full rounded-full bg-[var(--ribbon)] transition-all"
            style={{ width: `${pct}%` }}
          />
        </div>
        {pool && (
          <p className="num mt-2 text-center text-sm text-sub">
            {pool.donated.toLocaleString("zh-CN")} /{" "}
            {pool.goal.toLocaleString("zh-CN")} 火花（{pct}%）
          </p>
        )}
        {pool?.promo_started && (
          <p className="mt-2 text-center font-bold text-mint">
            双免促销进行中！
          </p>
        )}
      </section>

      {pool && pool.top_donors.length > 0 && (
        <section className="rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
          <h2 className="mb-2 font-display text-lg">本月捐赠榜</h2>
          <ol className="flex flex-col gap-1 text-sm">
            {pool.top_donors.map(([name, amount], i) => (
              <li key={name} className="flex justify-between">
                <span>
                  {i === 0 ? "🥇 " : i === 1 ? "🥈 " : i === 2 ? "🥉 " : ""}
                  {name}
                </span>
                <span className="num text-sub">
                  {amount.toLocaleString("zh-CN")}
                </span>
              </li>
            ))}
          </ol>
        </section>
      )}
    </div>
  );
}

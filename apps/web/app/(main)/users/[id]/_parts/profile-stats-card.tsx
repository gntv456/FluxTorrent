import { api } from "@/lib/api-client";
import { fmt } from "@/i18n/config";
import type { Dict } from "@/i18n/zh-CN";

interface Cat {
  id: number;
  name: string;
  count: number;
  bytes: number;
}
interface StatsResp {
  seeding: {
    bytes: number;
    count: number;
    hours: number;
    downloaded_done: number;
  };
  categories: Cat[];
  seed_buckets: { k: string; n: number }[];
}

const BUCKET_LABELS: Record<string, string> = {
  lt1h: "<1h",
  h1_24: "1-24h",
  d1_7: "1-7d",
  d7_30: "7-30d",
  gt30d: ">30d",
};

/** 他人主页统计卡（0503）：做种构成 + 类别热点 + 时长分布。
 *  服务端渲染纯 CSS 条形（与 usercp 趋势图同口径，零图表库）；
 *  数据空（无做种）时整卡自隐——不给围观者看空壳。 */
export async function ProfileStatsCard({
  uid,
  dict,
}: {
  uid: number;
  dict: Dict;
}) {
  const t = dict.profileStats;
  let s: StatsResp | null = null;
  try {
    s = await api.get<StatsResp>(`/api/v1/users/${uid}/stats`);
  } catch {
    s = null;
  }
  if (!s || s.seeding.count === 0) return null;

  const maxCat = Math.max(1, ...s.categories.map((c) => c.bytes));
  const maxBucket = Math.max(1, ...s.seed_buckets.map((b) => b.n));
  return (
    <section className="brand-panel p-4 flex flex-col gap-4">
      <div className="flex items-baseline justify-between">
        <h2 className="font-semibold">{t.title}</h2>
        <p className="text-xs opacity-60">
          {fmt(t.seedingLine, {
            n: s.seeding.count,
            size: fmtSize(s.seeding.bytes),
            hours: s.seeding.hours.toLocaleString(),
          })}
        </p>
      </div>

      <div>
        <p className="text-xs opacity-60 mb-1">{t.catsTitle}</p>
        <div className="flex flex-col gap-1.5">
          {s.categories.map((c) => (
            <div key={c.id} className="flex items-center gap-2 text-xs">
              <span className="w-20 shrink-0 truncate">{c.name}</span>
              <div
                className="h-2 rounded bg-[var(--accent)]"
                style={{ width: `${(c.bytes / maxCat) * 70}%` }}
              />
              <span className="opacity-70">
                {fmtSize(c.bytes)} · {c.count}
              </span>
            </div>
          ))}
        </div>
      </div>

      <div>
        <p className="text-xs opacity-60 mb-1">{t.bucketsTitle}</p>
        <div className="flex items-end gap-2">
          {s.seed_buckets.map((b) => (
            <div
              key={b.k}
              className="flex flex-col items-center gap-1 shrink-0"
              title={`${BUCKET_LABELS[b.k]} · ${b.n}`}
            >
              <span className="text-[10px] opacity-70">{b.n || ""}</span>
              <div
                className="heat-cell heat-cell--l3"
                style={{ width: "14px", height: `${4 + (b.n / maxBucket) * 36}px` }}
              />
              <span className="text-[10px] opacity-60">
                {BUCKET_LABELS[b.k]}
              </span>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function fmtSize(n: number): string {
  const gb = 1024 ** 3;
  if (n >= 1024 * gb) return `${(n / (1024 * gb)).toFixed(1)} TB`;
  if (n >= gb) return `${(n / gb).toFixed(0)} GB`;
  if (n >= 1024 ** 2) return `${(n / 1024 ** 2).toFixed(0)} MB`;
  return `${n}`;
}

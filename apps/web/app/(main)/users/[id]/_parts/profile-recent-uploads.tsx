/**
 * 用户公开主页·「近期发布」卡（从 profile-center.tsx 按域拆出，300 行门禁）。
 * B3-7：补「更新时间」列（接口 recent_uploads 有 created_at）；接口未下发
 * seeders/leechers，按纪律不造活种数——只补拿得到的字段。
 */
import Link from "next/link";
import type { Locale } from "@/i18n/config";
import { dateLocale } from "@/i18n/config";
import { Card } from "./profile-card";
import type { ProfileData } from "./profile-types";
import { Badge } from "@/components/ui/badge";

export function RecentUploadsCard({
  data,
  t,
  gb,
  locale,
  seedLabel,
}: {
  data: ProfileData;
  t: Record<string, string>;
  gb: (n: number) => string;
  locale: Locale;
  seedLabel: string;
}) {
  return (
    <Card title={t.recentUploads} icon="☰" full>
      <table className="nexus-table ux-recent">
        <thead>
          <tr>
            <th scope="col" className="colhead">
              {t.recentUploads}
            </th>
            <th scope="col" className="colhead w-20 text-right">
              {seedLabel}
            </th>
            <th scope="col" className="colhead w-28 text-right">
              {t.colSize}
            </th>
            <th scope="col" className="colhead w-24 text-right">
              {t.colTime}
            </th>
          </tr>
        </thead>
        <tbody>
          {data.recent_uploads.map((u) => (
            <tr key={u.id}>
              <td>
                <Link href={`/torrent/${u.id}`} className="font-bold">
                  {u.name}
                </Link>
                {u.small_descr && (
                  <p className="text-xs text-sub">{u.small_descr}</p>
                )}
              </td>
              <td className="num shrink-0 text-right">
                <Badge tone="neutral" size="sm">{u.seeders}</Badge>
              </td>
              <td className="num shrink-0 text-right">
                <Badge tone="neutral" size="sm">{gb(u.size)}</Badge>
              </td>
              <td className="num shrink-0 text-right text-sub">
                {new Date(u.created_at).toLocaleDateString(dateLocale(locale))}
              </td>
            </tr>
          ))}
          {data.recent_uploads.length === 0 && (
            <tr>
              <td colSpan={4} className="py-6 text-center text-sub">
                {t.noUploads}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </Card>
  );
}

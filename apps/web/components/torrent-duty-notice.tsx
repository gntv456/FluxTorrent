import Link from "next/link";
import type { Dict } from "@/i18n/zh-CN";
import { Badge } from "@/components/ui/badge";

/** PM2-1：下载前风险告知。PT 新用户最容易在「不知道 H&R」的情况下
 *  下载后删种而受罚；本卡把 obligations 前置到决策点，并给出求助入口。
 *  纯展示 + 站内链接，不新增请求、不改任何后端语义。 */
export function TorrentDutyNotice({
  dict,
  torrentId,
  promotionLabel,
}: {
  dict: Dict;
  torrentId: number;
  promotionLabel: string;
}) {
  const u = dict.uxOpt;
  return (
    <aside className="ux-duty" aria-labelledby="ux-duty-title">
      <h2 id="ux-duty-title" className="ux-duty__title">
        {u.dutyTitle}
      </h2>
      <ul className="ux-duty__list">
        <li>{u.dutyHr}</li>
        <li>{u.dutyKeep}</li>
      </ul>
      <p className="ux-duty__promo">
        <span>{u.dutyPromoLabel}</span>
        <Badge tone="brand" size="sm">{promotionLabel}</Badge>
      </p>
      <div className="ux-duty__links">
        <Link
          href="/help#survival"
          data-ux-event="guide_open"
          data-ux-label="torrent-duty"
        >
          {u.dutyLink} ›
        </Link>
        <Link href="/myhr">{u.dutyMyhr} ›</Link>
        <Link
          href={"/reports?type=torrent&id=" + torrentId}
          data-ux-event="report_open"
          data-ux-label="torrent-detail"
        >
          {u.reportTorrent} ›
        </Link>
      </div>
    </aside>
  );
}

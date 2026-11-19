import Link from "next/link";
import type { Dict } from "@/i18n/zh-CN";
import { Badge } from "@/components/ui/badge";

/** PM2-3：发布前自查。把审核退回最常见的四个原因前置到提交之前，
 *  纯提示、不阻断提交（阻断式清单会制造绕过行为，反而更糟）。 */
export function PublishChecklist({ dict }: { dict: Dict }) {
  const u = dict.uxOpt;
  const items = [
    u.publishCheck1,
    u.publishCheck2,
    u.publishCheck3,
    u.publishCheck4,
  ];
  return (
    <section className="ux-pcheck" aria-labelledby="ux-pcheck-title">
      <div className="ux-pcheck__head">
        <h2 id="ux-pcheck-title" className="ux-pcheck__title">
          {u.publishCheckTitle}
        </h2>
        <Badge tone="neutral" size="sm">{u.publishCheckHint}</Badge>
      </div>
      <ul className="ux-pcheck__list">
        {items.map((text, i) => (
          <li key={text} className="ux-pcheck__item">
            <span className="ux-pcheck__num" aria-hidden="true">{i + 1}</span>
            <span>{text}</span>
          </li>
        ))}
      </ul>
      <Link
        href="/help#survival"
        className="ux-pcheck__link"
        data-ux-event="guide_open"
        data-ux-label="publish-check"
      >
        {u.publishRulesLink} ›
      </Link>
    </section>
  );
}

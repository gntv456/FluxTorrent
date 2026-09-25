/**
 * 用户公开主页·「更多资料」卡（从 profile-center.tsx 按域拆出，300 行门禁）：
 * 发布数 / 评论数 / 论坛动态 / 字幕作品摘要 / 字幕身份徽章（0149）。
 * server component：数据与文案经 props 注入。
 */

import { Card, Row } from "./profile-card";
import type { ProfileData } from "./profile-types";

export function MoreInfoCard({
  data,
  t,
}: {
  data: ProfileData;
  t: Record<string, string>;
}) {
  const p = data.profile;
  return (
    <Card title={t.moreInfo} icon="◍">
      <Row label={t.uploads} icon="☰">
        <span className="num">{p.uploads}</span>
      </Row>
      <Row label={t.comments} icon="✎">
        <span className="num">{p.comments}</span>
      </Row>
      <Row label={t.tabPosts} icon="◈">
        <span className="num">{data.recent_posts.length}</span>
      </Row>
      {(data.user_fields ?? []).map((f) => (
        <Row key={f.key} label={f.label} icon="✦">
          {f.type === "multiselect" && Array.isArray(f.value)
            ? (f.value as string[]).join(" / ")
            : f.type === "bool"
              ? f.value
                ? "✓"
                : "—"
              : String(f.value ?? "—")}
        </Row>
      ))}
      {data.subtitle_count > 0 && (
        <Row label={t.subtitleWorks} icon="╬">
          <span className="num">
            {`${data.subtitle_count} · ${t.subtitleDownloads} ` +
              `${data.subtitle_downloads}`}
          </span>
        </Row>
      )}
      {data.subtitle_cert && (
        <Row label={t.subtitleCertLabel} icon="✎">
          <CertTierSpan tier={data.subtitle_cert} t={t} />
        </Row>
      )}
    </Card>
  );
}

/** 字幕身份徽章（0149：gold 金色 / certified 绿色） */
function CertTierSpan({
  tier,
  t,
}: {
  tier: "certified" | "gold";
  t: Record<string, string>;
}) {
  return (
    <span
      className={
        tier === "gold"
          ? "font-bold text-amber-500"
          : "font-bold text-mint"
      }
    >
      {tier === "gold"
        ? (t.subtitleCertGold ?? "gold")
        : (t.subtitleCertName ?? "certified")}
    </span>
  );
}

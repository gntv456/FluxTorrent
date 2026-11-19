/** 用户公开主页·个人中心卡片网格（自 page.tsx 拆出）：CenterGrid 账号/
 *  进度/传输做种/连接/资料/勋章/简介/发布。无 hooks。 */
import Link from "next/link";
import { Icon } from "@/components/icons";
import { MedalIcon } from "@/components/medal-icon";
import type { Locale } from "@/i18n/config";
import { dateLocale } from "@/i18n/config";
import { Card, Row } from "./profile-card";
import { MoreInfoCard } from "./profile-moreinfo";
import { RecentUploadsCard } from "./profile-recent-uploads";
import type { ProfileData } from "./profile-types";

export function CenterGrid({
  data,
  locale,
  t,
  ratio,
  realRatio,
  gb,
  seedDur,
  prog,
  seedLabel,
}: {
  data: ProfileData;
  locale: Locale;
  t: Record<string, string>;
  ratio: string;
  realRatio: string;
  gb: (n: number) => string;
  seedDur: (s: number) => string;
  prog: (
    cur: number,
    need: number,
    fmt: (n: number) => string,
  ) => { pct: number; text: string };
  seedLabel: string;
}) {
  const p = data.profile;
  const nc = data.next_class;
  return (
    <div className="up-grid">
      {/* —— 账号信息（首屏左列第一行） —— */}
      <Card title={t.accountInfo} icon="◈" slot="tl">
        <Row label="UID" icon="#">
          <span className="num">{p.id}</span>
        </Row>
        {data.inviter_name && (
          <Row label={t.inviter} icon="◑">
            <Link href={`/users/${data.inviter_id}`}>{data.inviter_name}</Link>
          </Row>
        )}
        <Row label={t.invitesPending} icon="✦">
          <span className="num">{data.invites_pending}</span>
        </Row>
        {data.gender && (
          <Row label={t.gender} icon="◆">
            {data.gender}
          </Row>
        )}
        {data.country && (
          <Row label={t.country} icon="◍">
            {data.country}
          </Row>
        )}
        {data.isp && (
          <Row label={t.isp} icon="⇄">
            {data.isp}
          </Row>
        )}
        <Row label={t.joined} icon="◷">
          <span className="num">
            {new Date(p.created_at).toLocaleDateString(dateLocale(locale))}
          </span>
        </Row>
        {p.last_seen_at && (
          <Row label={t.lastSeen} icon="◷">
            <span className="num">
              {new Date(p.last_seen_at).toLocaleString(dateLocale(locale))}
            </span>
          </Row>
        )}
        <Row label={t.medals} icon="✪">
          <span className="num">
            {`${p.medals}${data.achievements > 0 ? ` · ${t.achievements} ${data.achievements}` : ""}`}
          </span>
        </Row>
      </Card>

      {/* —— 下一级进度（首屏左列第二行：紧贴账号信息下方、传输与做种左侧；
             四项条件各一条，达标满格；最高级显示达顶） —— */}
      <Card title={t.nextLevel} icon="▲" slot="bl">
        {nc ? (
          <>
            <p className="mb-2 text-xs font-bold text-sub">{`→ ${nc.name}`}</p>
            <div className="flex flex-col gap-2 pb-2">
              {[
                {
                  label: t.upProgress,
                  ...prog(nc.uploaded, nc.uploaded_need, (n) => gb(n)),
                },
                {
                  label: t.dlProgress,
                  ...prog(nc.download_count, nc.download_count_need, (n) =>
                    String(n),
                  ),
                },
                {
                  label: t.seedProgress,
                  ...prog(
                    nc.seed_hours,
                    nc.seed_hours_need,
                    (n) => `${n} ${t.hours}`,
                  ),
                },
                {
                  label: t.ageProgress,
                  ...prog(
                    nc.account_age_days,
                    nc.account_age_days_need,
                    (n) => `${n} ${t.days}`,
                  ),
                },
              ].map((r) => (
                <div
                  key={r.label}
                  className="up-progress flex items-center gap-2 text-xs"
                >
                  <span className="up-progress__label w-16 shrink-0 text-sub">
                    {r.label}
                  </span>
                  <div className="up-progress__track h-2 flex-1 overflow-hidden rounded-full bg-[var(--surface-sunken)]">
                    <div
                      className={`h-full rounded-full ${r.pct >= 100 ? "bg-mint" : "bg-sky"}`}
                      style={{ width: `${r.pct}%` }}
                    />
                  </div>
                  <span className="up-progress__val num w-44 shrink-0 text-right text-sub">
                    {`${r.pct}% · ${r.text}`}
                  </span>
                </div>
              ))}
            </div>
          </>
        ) : (
          <p className="py-3 text-center text-xs text-sub">{t.maxLevel}</p>
        )}
      </Card>

      {/* —— 传输与做种（首屏右列，横跨两行，齐平左侧两张卡的总高） —— */}
      <Card title={t.transferSection} icon="⇅" slot="r2">
        <Row label={t.uploaded} icon="↑">
          <span className="num text-mint">{gb(p.uploaded)}</span>
        </Row>
        <Row label={t.downloaded} icon="↓">
          <span className="num text-coral">{gb(p.downloaded)}</span>
        </Row>
        <Row label={t.ratio} icon="≈">
          <span className="num font-bold">{ratio}</span>
        </Row>
        <Row label={t.realRatio} icon="≈">
          <span className="num">
            {`${realRatio}${t.realTraffic}: ${gb(data.real_uploaded)} / ${gb(data.real_downloaded)}`}
          </span>
        </Row>
        <Row label={t.seedSnatchCount} icon="◆">
          <span className="num">{`${p.seeding} / ${p.leeching} / ${data.completed_snatches}`}</span>
        </Row>
        <Row label={t.seedingSize} icon="▣">
          <span className="num">{gb(data.seeding_size)}</span>
        </Row>
        <Row label={t.seedTime} icon="◷">
          {/* 做种积分（2026-11-17）：title 说清=时长度量不能消费，
              收益引导到我的魔力页 */}
          <span
            className="num"
            title={data.seed_seconds >= 3600 ? t.seedPointsNote : undefined}
          >
            {seedDur(data.seed_seconds)}
            {seedPointsHint(data.seed_seconds, t.seedPointsUnit, locale)}
          </span>
        </Row>
        <Row label={t.hrField} icon="▲">
          <span
            className={`num ${data.hr_unresolved > 0 ? "font-bold text-coral" : ""}`}
          >
            {`${data.hr_unresolved} / ${data.hr_limit}`}
          </span>
        </Row>
        <Row label={t.sparkField} icon="✦">
          <span className="num">
            {Number(data.spark_balance).toLocaleString(dateLocale(locale))}
          </span>
        </Row>
        <Row label={t.seedEarnField} icon="★">
          <span className="num">
            {`${Number(data.month_seed_earn).toLocaleString(dateLocale(locale))}（${t.thisMonth}）`}
          </span>
        </Row>
      </Card>

      {/* —— 连接信息 —— */}
      <Card title={t.connectInfo} icon="⇄">
        {data.client_agent ? (
          <Row label={t.btClient} icon="⇄">
            <span className="text-xs">{data.client_agent}</span>
          </Row>
        ) : (
          <p className="up-note">{t.noClient}</p>
        )}
        {(data.upload_speed !== null || data.download_speed !== null) && (
          <Row label={t.speed} icon="⇅">
            {`${t.speedUp} ${data.upload_speed ?? 0} ${t.mbps} · ${t.speedDown} ${
              data.download_speed ?? 0
            } ${t.mbps}`}
          </Row>
        )}
      </Card>

      {/* —— 更多资料 —— */}
      <MoreInfoCard data={data} t={t} />

      {/* —— 佩戴勋章（展示位，整行：与上方双列错开，避免留出半格空位） —— */}
      <Card title={t.wornMedals} icon="✪" full>
        {data.worn_medals.length > 0 ? (
          <div className="up-medals">
            {data.worn_medals.map((m) => (
              <span
                key={m.id}
                title={m.description ?? m.name}
                className="flex items-center gap-1.5 rounded-full border border-line bg-[var(--surface-sunken)] px-2.5 py-0.5 text-xs font-bold text-ink"
              >
                <MedalIcon src={m.asset_ref} size={18} title={m.name} />
                {m.name}
              </span>
            ))}
          </div>
        ) : (
          <div className="ux-medal-empty">
            <p className="up-note">{t.noMedals}</p>
            <Link href="/medal-wall" className="ux-medal-cta">
              <Icon name="medal" size={16} />
              <span>{t.wornMedals}</span>
              <Icon name="chevronRight" size={14} />
            </Link>
          </div>
        )}
      </Card>

      {/* —— 个人简介 / 签名档（有值才展示，整行宽度） —— */}
      {data.info && (
        <Card title={t.infoTitle} icon="✎" full>
          <p className="whitespace-pre-wrap py-2 text-sm">{data.info}</p>
        </Card>
      )}
      {data.signature && (
        <Card title={t.signatureTitle} icon="❞" full>
          <p className="whitespace-pre-wrap border-l-2 border-line py-2 pl-3 text-sm text-sub italic">
            {data.signature}
          </p>
        </Card>
      )}

      {/* B3-7：近期发布（补更新时间列，见 _parts/profile-recent-uploads.tsx） */}
      <RecentUploadsCard
        data={data}
        t={t}
        gb={gb}
        locale={locale}
        seedLabel={seedLabel}
      />
    </div>
  );
}

/** 做种积分副注：1 积分 = 1 小时（P1-6）；<1h 不显示防「≈0」挫败感。 */
function seedPointsHint(secs: number, unit: string, locale: Locale): string {
  if (secs < 3600) return "";
  const pts = Math.floor(secs / 3600).toLocaleString(dateLocale(locale));
  return ` · ≈${pts} ${unit}`;
}

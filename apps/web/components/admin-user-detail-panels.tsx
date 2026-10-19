"use client";

/**
 * 后台用户详情·资料全景与关联数据 tab（从 components/admin-user-detail.tsx 按域拆出）：
 * ProfilePanels 账号资料 + 数据统计、SparkPanel 火花流水、LoginsPanel 登录记录、
 * SeedingPanel 做种/下载列表。数据经 props 注入，分页控件随行渲染。
 */

import type {
  Detail,
  LoginRow,
  SeedRow,
  SparkRow,
} from "./admin-user-detail-shared";
import { fmtBytes, fmtHours } from "./admin-user-detail-shared";
import { PagedBar } from "./admin-user-detail-pager";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { sparkKindLabels } from "@/lib/spark-kind-labels";

export function ProfilePanels({
  d,
  currency,
  dt,
}: {
  d: Detail;
  currency: string;
  dt: (s: string | null) => string;
}) {
  const { dict } = useI18n();
  const u = dict.userDetail;
  return (
    <>
      <section className="baozi-panel p-4">
        <h2 className="mb-2 text-base font-bold text-ink">{u.profileTitle}</h2>
        <dl className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm md:grid-cols-3">
          <div>
            <dt className="text-sub">{u.email}</dt>
            <dd>{d.email}</dd>
          </div>
          <div>
            <dt className="text-sub">Passkey</dt>
            <dd className="font-mono text-xs">
              {d.passkey_masked}
            </dd>
          </div>
          <div>
            <dt className="text-sub">{u.twoFa}</dt>
            <dd>{d.totp_enabled ? u.totpOn : u.totpOff}</dd>
          </div>
          <div>
            <dt className="text-sub">{u.inviter}</dt>
            <dd>
              {d.inviter_name ? `${d.inviter_name} (#${d.invited_by})` : "—"}
            </dd>
          </div>
          <div>
            <dt className="text-sub">{u.createdAt}</dt>
            <dd>{dt(d.created_at)}</dd>
          </div>
          <div>
            <dt className="text-sub">{u.lastSeen}</dt>
            <dd>{dt(d.last_seen_at)}</dd>
          </div>
          <div>
            <dt className="text-sub">{u.lastIp}</dt>
            <dd>{d.last_ip ?? "—"}</dd>
          </div>
          <div>
            <dt className="text-sub">{u.invitesUnused}</dt>
            <dd>{d.invites_unused}</dd>
          </div>
          <div>
            <dt className="text-sub">{u.attendDays}</dt>
            <dd>{d.attendance_days}</dd>
          </div>
          {d.warned_until && (
            <div className="md:col-span-3">
              <dt className="text-sub">{u.warn}</dt>
              <dd className="text-danger">
                {fmt(u.warnUntil, { date: dt(d.warned_until) })} ·{" "}
                {d.warned_reason ?? u.noReason}
              </dd>
            </div>
          )}
        </dl>
      </section>

      <section className="baozi-panel p-4">
        <h2 className="mb-2 text-base font-bold text-ink">{u.statsTitle}</h2>
        <dl className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm md:grid-cols-4">
          {/* 上传/下载/分享率/魔力余额已上移资料 Hero（对齐样图 § 用户详情
              的头条统计位），不是删字段——Hero 与页面头仍在原处展示。 */}
          <div>
            <dt className="text-sub">{u.uploadsCount}</dt>
            <dd>{d.uploads}</dd>
          </div>
          <div>
            <dt className="text-sub">{u.downloadsCount}</dt>
            <dd>{d.downloaded_count}</dd>
          </div>
          <div>
            <dt className="text-sub">{u.comments}</dt>
            <dd>{d.comments}</dd>
          </div>
          <div>
            <dt className="text-sub">{u.medals}</dt>
            <dd>{d.medals}</dd>
          </div>
          <div>
            <dt className="text-sub">{u.seedingNow}</dt>
            <dd>{d.seeding}</dd>
          </div>
          <div>
            <dt className="text-sub">{u.leechingNow}</dt>
            <dd>{d.leeching}</dd>
          </div>
          <div>
            <dt className="text-sub">{u.seedHoursTotal}</dt>
            <dd>{fmtHours(d.seed_seconds, u.hoursUnit)}</dd>
          </div>
        </dl>
      </section>
    </>
  );
}

export function SparkPanel({
  spark,
  page,
  setPage,
  dt,
}: {
  spark: { rows: SparkRow[]; total: number } | null;
  page: number;
  setPage: (v: number) => void;
  dt: (s: string | null) => string;
}) {
  const dl = useI18n().dict;
  const u = dl.userDetail;
  const kindLabels = sparkKindLabels(dl.my);
  return (
    <section className="baozi-panel flex flex-col gap-2 p-4">
      <PagedBar total={spark?.total ?? 0} cur={page} onPage={setPage} />
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">{u.sparkChange}</td>
              <td className="colhead">{u.sparkKind}</td>
              <td className="colhead">{u.balanceAfter}</td>
              <td className="colhead">{u.time}</td>
            </tr>
          </thead>
          <tbody>
            {spark?.rows.map((r) => (
              <tr key={r.id}>
                <td
                  className={`num font-bold ${r.amount >= 0 ? "text-mint" : "text-danger"}`}
                >
                  {r.amount >= 0 ? "+" : ""}
                  {r.amount.toLocaleString()}
                </td>
                <td>{kindLabels[r.kind] ?? r.kind}</td>
                <td className="num">{r.balance_after.toLocaleString()}</td>
                <td className="text-sub">{dt(r.created_at)}</td>
              </tr>
            ))}
            {spark?.rows.length === 0 && (
              <tr>
                <td colSpan={4} className="py-4 text-center text-sub">
                  {u.sparkEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}

export function LoginsPanel({
  logins,
  page,
  setPage,
  dt,
}: {
  logins: { rows: LoginRow[]; total: number } | null;
  page: number;
  setPage: (v: number) => void;
  dt: (s: string | null) => string;
}) {
  const u = useI18n().dict.userDetail;
  return (
    <section className="baozi-panel flex flex-col gap-2 p-4">
      <PagedBar total={logins?.total ?? 0} cur={page} onPage={setPage} />
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">IP</td>
              <td className="colhead">{u.result}</td>
              <td className="colhead">{u.time}</td>
            </tr>
          </thead>
          <tbody>
            {logins?.rows.map((r) => (
              <tr key={r.id}>
                <td className="font-mono">{r.ip}</td>
                <td className={r.ok ? "text-mint" : "text-danger"}>
                  {r.ok ? u.loginOk : u.loginFail}
                </td>
                <td className="text-sub">{dt(r.created_at)}</td>
              </tr>
            ))}
            {logins?.rows.length === 0 && (
              <tr>
                <td colSpan={3} className="py-4 text-center text-sub">
                  {u.loginsEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}

export function SeedingPanel({ seeds }: { seeds: SeedRow[] | null }) {
  const u = useI18n().dict.userDetail;
  return (
    <section className="baozi-panel p-4">
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">{u.seedColTorrent}</td>
              <td className="colhead w-24">{u.seedColSize}</td>
              <td className="colhead w-24">{u.seedColStatus}</td>
              <td className="colhead w-24">{u.seedColHours}</td>
              <td className="colhead w-16">H&R</td>
            </tr>
          </thead>
          <tbody>
            {seeds?.map((s) => (
              <tr key={s.torrent_id}>
                <td>
                  <a className="text-link" href={`/torrent/${s.torrent_id}`}>
                    {s.name}
                  </a>
                </td>
                <td className="num">{fmtBytes(s.size)}</td>
                <td>
                  {s.seeding ? (
                    <span className="text-mint">{u.seedingNow}</span>
                  ) : (
                    <span className="text-sub">{u.stopped}</span>
                  )}
                </td>
                <td className="num">
                  {fmtHours(s.seeded_seconds, u.hoursUnit)}
                </td>
                <td>
                  {s.hr_flag ? (
                    <span className="text-danger">{u.hrHit}</span>
                  ) : (
                    "—"
                  )}
                </td>
              </tr>
            ))}
            {seeds?.length === 0 && (
              <tr>
                <td colSpan={5} className="py-4 text-center text-sub">
                  {u.seedsEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}

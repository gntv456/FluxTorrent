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
import { fmtBytes, fmtHours, PER_PAGE } from "./admin-user-detail-shared";

export function PagedBar({
  total,
  cur,
  onPage,
}: {
  total: number;
  cur: number;
  onPage: (v: number) => void;
}) {
  return (
    <div className="flex items-center justify-between text-xs text-sub">
      <span>共 {total} 条</span>
      <div className="flex items-center gap-2">
        <button disabled={cur <= 1} onClick={() => onPage(cur - 1)} className="min-h-[32px] rounded-full border border-line px-3 disabled:opacity-40">上一页</button>
        <span>{cur} / {Math.max(1, Math.ceil(total / PER_PAGE))}</span>
        <button disabled={cur >= Math.ceil(total / PER_PAGE)} onClick={() => onPage(cur + 1)} className="min-h-[32px] rounded-full border border-line px-3 disabled:opacity-40">下一页</button>
      </div>
    </div>
  );
}

export function ProfilePanels({
  d,
  currency,
  dt,
}: {
  d: Detail;
  currency: string;
  dt: (s: string | null) => string;
}) {
  return (
    <>
      <section className="baozi-panel p-4">
        <h2 className="mb-2 text-base font-bold text-ink">账号资料</h2>
        <dl className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm md:grid-cols-3">
          <div><dt className="text-sub">邮箱</dt><dd>{d.email}</dd></div>
          <div><dt className="text-sub">Passkey</dt><dd className="font-mono text-xs">{d.passkey.slice(0, 8)}…{d.passkey.slice(-4)}</dd></div>
          <div><dt className="text-sub">两步验证</dt><dd>{d.totp_enabled ? "已开启" : "未开启"}</dd></div>
          <div><dt className="text-sub">邀请人</dt><dd>{d.inviter_name ? `${d.inviter_name} (#${d.invited_by})` : "—"}</dd></div>
          <div><dt className="text-sub">添加时间</dt><dd>{dt(d.created_at)}</dd></div>
          <div><dt className="text-sub">最后访问</dt><dd>{dt(d.last_seen_at)}</dd></div>
          <div><dt className="text-sub">最近登录 IP</dt><dd>{d.last_ip ?? "—"}</dd></div>
          <div><dt className="text-sub">未用邀请</dt><dd>{d.invites_unused}</dd></div>
          <div><dt className="text-sub">签到天数</dt><dd>{d.attendance_days}</dd></div>
          {d.warned_until && (
            <div className="md:col-span-3"><dt className="text-sub">警告</dt><dd className="text-danger">至 {dt(d.warned_until)} · {d.warned_reason ?? "无理由记录"}</dd></div>
          )}
        </dl>
      </section>

      <section className="baozi-panel p-4">
        <h2 className="mb-2 text-base font-bold text-ink">数据统计</h2>
        <dl className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm md:grid-cols-4">
          <div><dt className="text-sub">上传量</dt><dd>{fmtBytes(d.uploaded)}</dd></div>
          <div><dt className="text-sub">下载量</dt><dd>{fmtBytes(d.downloaded)}</dd></div>
          <div><dt className="text-sub">分享率</dt><dd>{d.downloaded > 0 ? (d.uploaded / d.downloaded).toFixed(3) : "∞"}</dd></div>
          <div><dt className="text-sub">{currency}余额</dt><dd>{d.spark_balance.toLocaleString()}</dd></div>
          <div><dt className="text-sub">发布种子</dt><dd>{d.uploads}</dd></div>
          <div><dt className="text-sub">完成下载</dt><dd>{d.downloaded_count}</dd></div>
          <div><dt className="text-sub">评论</dt><dd>{d.comments}</dd></div>
          <div><dt className="text-sub">勋章</dt><dd>{d.medals}</dd></div>
          <div><dt className="text-sub">做种中</dt><dd>{d.seeding}</dd></div>
          <div><dt className="text-sub">下载中</dt><dd>{d.leeching}</dd></div>
          <div><dt className="text-sub">做种总时长</dt><dd>{fmtHours(d.seed_seconds)}</dd></div>
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
  return (
    <section className="baozi-panel flex flex-col gap-2 p-4">
      <PagedBar total={spark?.total ?? 0} cur={page} onPage={setPage} />
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead><tr>
            <td className="colhead">变动</td><td className="colhead">类型</td>
            <td className="colhead">变动后余额</td><td className="colhead">时间</td>
          </tr></thead>
          <tbody>
            {spark?.rows.map((r) => (
              <tr key={r.id}>
                <td className={`num font-bold ${r.amount >= 0 ? "text-mint" : "text-danger"}`}>{r.amount >= 0 ? "+" : ""}{r.amount.toLocaleString()}</td>
                <td>{r.kind}</td>
                <td className="num">{r.balance_after.toLocaleString()}</td>
                <td className="text-sub">{dt(r.created_at)}</td>
              </tr>
            ))}
            {spark?.rows.length === 0 && <tr><td colSpan={4} className="py-4 text-center text-sub">暂无流水</td></tr>}
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
  return (
    <section className="baozi-panel flex flex-col gap-2 p-4">
      <PagedBar total={logins?.total ?? 0} cur={page} onPage={setPage} />
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead><tr>
            <td className="colhead">IP</td><td className="colhead">结果</td><td className="colhead">时间</td>
          </tr></thead>
          <tbody>
            {logins?.rows.map((r) => (
              <tr key={r.id}>
                <td className="font-mono">{r.ip}</td>
                <td className={r.ok ? "text-mint" : "text-danger"}>{r.ok ? "成功" : "失败"}</td>
                <td className="text-sub">{dt(r.created_at)}</td>
              </tr>
            ))}
            {logins?.rows.length === 0 && <tr><td colSpan={3} className="py-4 text-center text-sub">暂无登录记录</td></tr>}
          </tbody>
        </table>
      </div>
    </section>
  );
}

export function SeedingPanel({ seeds }: { seeds: SeedRow[] | null }) {
  return (
    <section className="baozi-panel p-4">
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead><tr>
            <td className="colhead">种子</td><td className="colhead w-24">大小</td>
            <td className="colhead w-24">状态</td><td className="colhead w-24">做种时长</td>
            <td className="colhead w-16">H&R</td>
          </tr></thead>
          <tbody>
            {seeds?.map((s) => (
              <tr key={s.torrent_id}>
                <td><a className="text-link" href={`/torrent/${s.torrent_id}`}>{s.name}</a></td>
                <td className="num">{fmtBytes(s.size)}</td>
                <td>{s.seeding ? <span className="text-mint">做种中</span> : <span className="text-sub">已停</span>}</td>
                <td className="num">{fmtHours(s.seeded_seconds)}</td>
                <td>{s.hr_flag ? <span className="text-danger">命中</span> : "—"}</td>
              </tr>
            ))}
            {seeds?.length === 0 && <tr><td colSpan={5} className="py-4 text-center text-sub">暂无做种/下载记录</td></tr>}
          </tbody>
        </table>
      </div>
    </section>
  );
}

"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface TaskRow {
  id: number;
  name: string;
  metric: {
    upload_delta?: number;
    download_delta?: number;
    seed_points_delta?: number;
    uploads?: number;
    subtitles?: number;
    [k: string]: unknown;
  };
  reward: number;
  penalty: number;
  claim_limit: number | null;
  claimed: number;
  starts_at: string;
  ends_at: string;
  tier?: string;
  subtitle?: string;
  fee?: number;
  duration_days?: number;
  quota_total?: number;
  claimed_by_me?: boolean;
}

interface TaskOverview {
  shop: { name: string; span: string; require_tier: string; require_count: number; cost: number; stock: number }[];
  feed: { user: string; task: string; at: string }[];
  stats: {
    ongoing: number;
    done: number;
    failed: number;
    tiers: { tier: string | null; total: number; done: number; pct: number }[];
  };
  my_records: { task_id: number; name: string; status: number; claimed_at: string; settled_at: string | null }[];
}

function fmtBytes(n: number): string {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(2)} ${units[i]}`;
}

/** 任务系统（参考站 task.php 复刻）：TASK SYSTEM hero + 01 规则 + 02 可领取任务（五档卡）
 *  + 03 任务商店 + 06 最新动态 + 07 任务统计 + 08 我的任务记录 */
export function TaskBoard({ sparkBalance }: { sparkBalance: number | null }) {
  const { dict } = useI18n();
  const t = dict.tasks2;
  const [tasks, setTasks] = useState<TaskRow[] | null>(null);
  const [ov, setOv] = useState<TaskOverview | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<number | null>(null);

  const load = async () => {
    try {
      const [tl, o] = await Promise.all([
        api.get<TaskRow[]>("/api/v1/tasks"),
        api.get<TaskOverview>("/api/v1/tasks/overview"),
      ]);
      setTasks(tl);
      setOv(o);
    } catch {
      setTasks([]);
    }
  };
  useEffect(() => {
    load();
  }, []);

  async function claim(id: number) {
    setBusyId(id);
    setMsg(null);
    try {
      await api.post("/api/v1/tasks/claim", { task_id: id });
      setMsg(t.claimOk);
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusyId(null);
    }
  }

  const tierTasks = (tasks ?? []).filter((x) => x.tier);
  const otherTasks = (tasks ?? []).filter((x) => !x.tier);

  return (
    <div className="task-page">
      {/* Hero */}
      <section className="task-hero">
        <div>
          <span className="task-eyebrow">{dict.common.brand.toUpperCase()} TASK SYSTEM</span>
          <h1>{t.title}</h1>
          <p>{t.subtitle}</p>
        </div>
        <div className="task-hero-meta">
          <div>
            <span>{t.currentSpark}</span>
            <strong className="num">{sparkBalance?.toFixed(1) ?? "—"}</strong>
          </div>
          <div>
            <span>{t.settleMode}</span>
            <strong>LIVE</strong>
          </div>
        </div>
      </section>

      {/* 01 规则 */}
      <section className="task-panel task-rules">
        <div className="task-section-heading">
          <div>
            <span>01</span>
            <h2>{t.rulesTitle}</h2>
          </div>
        </div>
        <ol>
          {t.rules.map((r) => (
            <li key={r}>{r}</li>
          ))}
        </ol>
      </section>

      {/* 02 可领取任务（五档卡） */}
      <section className="task-panel task-tier-section">
        <div className="task-section-heading">
          <div>
            <span>02</span>
            <h2>{t.tiersTitle}</h2>
          </div>
          <p>
            {t.quota}: {tierTasks.reduce((a, x) => a + x.claimed, 0)} /{" "}
            {tierTasks[0]?.quota_total ?? 200}；{t.vipNote}
          </p>
        </div>
        <div className="task-card-grid">
          {tierTasks.map((task) => (
            <article key={task.id} className={`task-tier-card task-tier-card--${task.tier}`}>
              <header>
                <h3>{task.name}</h3>
                <span>{task.subtitle}</span>
              </header>
              <dl>
                <div>
                  <dt>{t.realUpload}</dt>
                  <dd className="num">{fmtBytes(task.metric.upload_delta ?? 0)}</dd>
                </div>
                <div>
                  <dt>{t.realDownload}</dt>
                  <dd className="num">{fmtBytes(task.metric.download_delta ?? 0)}</dd>
                </div>
                <div>
                  <dt>{t.seedPoints}</dt>
                  <dd className="num">{(task.metric.seed_points_delta ?? 0).toLocaleString()}</dd>
                </div>
                <div>
                  <dt>{t.duration}</dt>
                  <dd className="num">
                    {task.duration_days ?? 30} {dict.usercp.days}
                  </dd>
                </div>
                <div>
                  <dt>{t.reward}</dt>
                  <dd className="num">{(task.reward ?? 0).toFixed(1)}</dd>
                </div>
                <div>
                  <dt>{t.penalty}</dt>
                  <dd className="num">{(task.penalty ?? 0).toFixed(1)}</dd>
                </div>
                <div>
                  <dt>{t.fee}</dt>
                  <dd className="num">{(task.fee ?? 0).toFixed(1)}</dd>
                </div>
              </dl>
              <button
                type="button"
                className="baozi-button"
                disabled={busyId === task.id || task.claimed_by_me}
                onClick={() => claim(task.id)}
              >
                {task.claimed_by_me ? t.claimedByMe : t.claimBtn}
              </button>
            </article>
          ))}
          {otherTasks.map((task) => (
            <article key={task.id} className="task-tier-card">
              <header>
                <h3>{task.name}</h3>
                <span>
                  {t.claimedPrefix} {task.claimed}
                  {task.claim_limit ? ` / ${task.claim_limit}` : ""}
                </span>
              </header>
              <dl>
                <div>
                  <dt>{t.reward}</dt>
                  <dd className="num">{task.reward.toLocaleString()}</dd>
                </div>
                <div>
                  <dt>{t.penalty}</dt>
                  <dd className="num">{task.penalty.toLocaleString()}</dd>
                </div>
              </dl>
              <button
                type="button"
                className="baozi-button"
                disabled={busyId === task.id || task.claimed_by_me}
                onClick={() => claim(task.id)}
              >
                {task.claimed_by_me ? t.claimedByMe : t.claimBtn}
              </button>
            </article>
          ))}
        </div>
      </section>

      {/* 03 任务商店 */}
      <section className="task-panel task-shop-section">
        <div className="task-section-heading">
          <div>
            <span>03</span>
            <h2>{t.shopTitle}</h2>
          </div>
        </div>
        <div className="task-card-grid task-shop-grid">
          {(ov?.shop ?? []).map((s, i) => (
            <article key={i} className="task-shop-card">
              <header>
                <h3>{s.name}</h3>
                <span>{s.span}</span>
              </header>
              <dl>
                <div>
                  <dt>{t.shopRequire}</dt>
                  <dd>
                    {s.require_tier.toUpperCase()} × {s.require_count}
                  </dd>
                </div>
                <div>
                  <dt>{t.shopDone}</dt>
                  <dd className="num">0</dd>
                </div>
                <div>
                  <dt>{t.shopAvail}</dt>
                  <dd className="num">0</dd>
                </div>
                <div>
                  <dt>{t.shopCost}</dt>
                  <dd className="num">{s.cost.toFixed(1)}</dd>
                </div>
                <div>
                  <dt>{t.shopStock}</dt>
                  <dd className="num">{s.stock}</dd>
                </div>
              </dl>
              <button type="button" className="baozi-button" disabled title={t.shopDisabled}>
                {t.shopBuy}
              </button>
            </article>
          ))}
        </div>
      </section>

      {/* 06 最新动态 + 07 任务统计 */}
      <div className="task-dashboard-grid">
        <section className="task-panel">
          <div className="task-section-heading">
            <div>
              <span>06</span>
              <h2>{t.feedTitle}</h2>
            </div>
          </div>
          <div className="task-feed">
            {(ov?.feed ?? []).map((f, i) => (
              <div key={i}>
                <p>
                  <strong>{f.user}</strong> {t.feedAction} {f.task}
                </p>
                <time>{new Date(f.at).toLocaleDateString("zh-CN", { month: "2-digit", day: "2-digit" })}{" "}
                  {new Date(f.at).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })}</time>
              </div>
            ))}
            {(ov?.feed ?? []).length === 0 && <p className="text-sub">{t.feedEmpty}</p>}
          </div>
        </section>
        <section className="task-panel">
          <div className="task-section-heading">
            <div>
              <span>07</span>
              <h2>{t.statsTitle}</h2>
            </div>
          </div>
          <div className="task-stats-summary">
            <div>
              <strong className="num">{ov?.stats.ongoing ?? 0}</strong>
              <span>{t.stOngoing}</span>
            </div>
            <div>
              <strong className="num">{ov?.stats.done ?? 0}</strong>
              <span>{t.stDone}</span>
            </div>
            <div>
              <strong className="num">{ov?.stats.failed ?? 0}</strong>
              <span>{t.stFailed}</span>
            </div>
          </div>
          <div className="task-stats-bars">
            {(ov?.stats.tiers ?? []).map((s) => (
              <div key={s.tier ?? "?"}>
                <span>{(s.tier ?? "?").charAt(0).toUpperCase() + (s.tier ?? "?").slice(1)}</span>
                <progress max={100} value={s.pct} />
                <b className="num">{s.pct.toFixed(1)}%</b>
              </div>
            ))}
          </div>
        </section>
      </div>

      {/* 08 我的任务记录 */}
      <section className="task-panel task-history">
        <div className="task-section-heading">
          <div>
            <span>08</span>
            <h2>{t.historyTitle}</h2>
          </div>
        </div>
        <div className="baozi-wide-table-scroll">
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">{t.hTask}</td>
                <td className="colhead">{t.hStatus}</td>
                <td className="colhead">{t.hClaimedAt}</td>
                <td className="colhead">{t.hSettledAt}</td>
              </tr>
              {(ov?.my_records ?? []).map((r, i) => (
                <tr key={i}>
                  <td>{r.name}</td>
                  <td>
                    {r.status === 0 ? t.stOngoing : r.status === 1 ? t.stDone : t.stFailed}
                  </td>
                  <td>{new Date(r.claimed_at).toLocaleString("zh-CN")}</td>
                  <td>{r.settled_at ? new Date(r.settled_at).toLocaleString("zh-CN") : "—"}</td>
                </tr>
              ))}
              {(ov?.my_records ?? []).length === 0 && (
                <tr>
                  <td colSpan={4} className="py-6 text-center text-sub">
                    {t.historyEmpty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </section>

      {msg && <p className="task-msg">{msg}</p>}
    </div>
  );
}

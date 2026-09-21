"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import {
  fmtBytes,
  tierLabel,
  type TaskRow,
  type TaskOverview,
  TaskDashboard,
  TaskHistory,
} from "@/components/task-board-parts";

// 任务系统（参考站 task.php 复刻）：TASK SYSTEM hero + 01 规则 + 02 可领取任务（五档卡）
// + 03 任务商店 + 06 最新动态 + 07 任务统计 + 08 我的任务记录。
// 数据契约/进度工具/08 记录子面板已按域拆出 @/components/task-board-parts。

export function TaskBoard({ sparkBalance }: { sparkBalance: number | null }) {
  const { dict, locale, currency } = useI18n();
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
          <span className="task-eyebrow">{t.heroEyebrow}</span>
          <h1>{t.title}</h1>
          <p>{t.subtitle}</p>
        </div>
        <div className="task-hero-meta">
          <div>
            <span>{t.currentSpark.replace("{magic}", currency)}</span>
            <strong className="num">{sparkBalance?.toFixed(1) ?? "—"}</strong>
          </div>
          <div>
            <span>{t.settleMode}</span>
            <strong>{t.settleLive}</strong>
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
            {/* 0094：quota_total 由后端 COALESCE(claim_limit, quota_total) 下发，运营改 claim_limit 即生效 */}
            {t.quota}: {tierTasks.reduce((a, x) => a + x.claimed, 0)} /{" "}
            {tierTasks[0]?.quota_total ?? 200}；{t.vipNote}
          </p>
        </div>
        <div className="task-card-grid">
          {tierTasks.map((task) => (
            <article
              key={task.id}
              className={`task-tier-card task-tier-card--${task.tier}`}
            >
              <header>
                <h3>{tierLabel(task.tier)}</h3>
                <span>{task.subtitle}</span>
              </header>
              <dl>
                <div>
                  <dt>{t.realUpload}</dt>
                  <dd className="num">
                    {fmtBytes(task.metric.upload_delta ?? 0)}
                  </dd>
                </div>
                <div>
                  <dt>{t.realDownload}</dt>
                  <dd className="num">
                    {fmtBytes(task.metric.download_delta ?? 0)}
                  </dd>
                </div>
                <div>
                  <dt>{t.seedPoints}</dt>
                  <dd className="num">
                    {(task.metric.seed_points_delta ?? 0).toLocaleString()}
                  </dd>
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
                    {tierLabel(s.require_tier)} × {s.require_count}
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
              <button
                type="button"
                className="baozi-button"
                disabled
                title={t.shopDisabled}
              >
                {t.shopBuy}
              </button>
            </article>
          ))}
        </div>
      </section>

      {/* 06 最新动态 + 07 任务统计 */}
      <TaskDashboard ov={ov} />

      {/* 08 我的任务记录 */}
      <TaskHistory ov={ov} />

      {msg && <p className="task-msg">{msg}</p>}
    </div>
  );
}

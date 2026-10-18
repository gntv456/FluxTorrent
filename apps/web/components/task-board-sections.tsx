"use client";

import { useI18n } from "@/i18n/client";
import {
  fmtBytes,
  tierLabel,
  type TaskRow,
  type TaskOverview,
} from "@/components/task-board-parts";

/** 任务系统的三个标签面板（从 task-board.tsx 按域拆出）：
 *  01 规则 / 02 可领取任务 / 03 任务商店。
 *
 *  ⚠️ 三块都**不再自带区块标题**：标题层级由 Tabs 的标签承担
 *  （评审文档「区块标题层级唯一」），样图 § tplTask 的面板里
 *  同样只有内容、没有第二级 h2。 */

export function TaskRulesPanel() {
  const { dict } = useI18n();
  const t = dict.tasks2;
  return (
    <section className="task-panel task-rules">
      <ol>
        {t.rules.map((r) => (
          <li key={r}>{r}</li>
        ))}
      </ol>
    </section>
  );
}

export function TaskTierPanel({
  tasks,
  busyId,
  onClaim,
}: {
  tasks: TaskRow[];
  busyId: number | null;
  onClaim: (id: number) => void;
}) {
  const { dict } = useI18n();
  const t = dict.tasks2;
  const tierTasks = tasks.filter((x) => x.tier);
  const otherTasks = tasks.filter((x) => !x.tier);
  return (
    <section className="task-panel">
      <p className="task-quota-note">
        {t.quota}: {tierTasks.reduce((a, x) => a + x.claimed, 0)} /{" "}
        {tierTasks[0]?.quota_total ?? 200}；{t.vipNote}
      </p>
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
              className="brand-button"
              disabled={busyId === task.id || task.claimed_by_me}
              onClick={() => onClaim(task.id)}
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
              className="brand-button"
              disabled={busyId === task.id || task.claimed_by_me}
              onClick={() => onClaim(task.id)}
            >
              {task.claimed_by_me ? t.claimedByMe : t.claimBtn}
            </button>
          </article>
        ))}
      </div>
    </section>
  );
}

export function TaskShopPanel({
  shop,
}: {
  shop: TaskOverview["shop"];
}) {
  const { dict } = useI18n();
  const t = dict.tasks2;
  return (
    <section className="task-panel">
      <div className="task-card-grid task-shop-grid">
        {(shop ?? []).map((s, i) => (
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
              className="brand-button"
              disabled
              title={t.shopDisabled}
            >
              {t.shopBuy}
            </button>
          </article>
        ))}
      </div>
    </section>
  );
}

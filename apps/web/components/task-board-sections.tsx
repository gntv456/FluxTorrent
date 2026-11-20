"use client";

import { useState } from "react";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
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
  const { dict, currency } = useI18n();
  const t = dict.tasks2;
  // P0：报名费扣款 + 30 天不可取消 + 失败扣罚，必须二次确认
  const [pending, setPending] = useState<TaskRow | null>(null);
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
              <h3>{tierLabel(task.tier, t)}</h3>
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
              onClick={() => setPending(task)}
            >
              {task.claimed_by_me ? t.claimedByMe : t.claimBtn}
              {/* 利益前置（2026-11-15）：奖励额直接标上按钮 */}
              {!task.claimed_by_me && task.reward ? (
                <i
                  className={
                    "ml-1 rounded-[6px] bg-white/25 px-1 py-px " +
                    "text-[11px] not-italic"
                  }
                >
                  +{Number(task.reward).toFixed(0)}
                </i>
              ) : null}
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
              onClick={() => setPending(task)}
            >
              {task.claimed_by_me ? t.claimedByMe : t.claimBtn}
              {/* 利益前置（2026-11-15）：奖励额直接标上按钮 */}
              {!task.claimed_by_me && task.reward ? (
                <i
                  className={
                    "ml-1 rounded-[6px] bg-white/25 px-1 py-px " +
                    "text-[11px] not-italic"
                  }
                >
                  +{Number(task.reward).toFixed(0)}
                </i>
              ) : null}
            </button>
          </article>
        ))}
      </div>
      <ConfirmDialog
        open={pending !== null}
        onClose={() => setPending(null)}
        onConfirm={() => {
          if (pending) onClaim(pending.id);
          setPending(null);
        }}
        tone="danger"
        title={dict.review.taskClaimTitle}
        message={fmt(dict.review.taskClaimBody, {
          fee: (pending?.fee ?? 0).toFixed(1),
          magic: currency,
          days: pending?.duration_days ?? 30,
          punish: (pending?.penalty ?? 0).toFixed(1),
        })}
        confirmLabel={dict.review.taskClaimConfirm}
        busy={pending !== null && busyId === pending.id}
      />
    </section>
  );
}

export function TaskShopPanel({
  shop,
  stats,
  onBuy,
  busyId,
}: {
  shop: TaskOverview["shop"];
  /** 各档真实完成数（GET /tasks/overview 的 stats.tiers） */
  stats?: TaskOverview["stats"];
  onBuy: (item: TaskOverview["shop"][number]) => void;
  busyId: number | null;
}) {
  const { dict, currency } = useI18n();
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
                  {tierLabel(s.require_tier, t)} × {s.require_count}
                </dd>
              </div>
              <div>
                <dt>{t.shopDone}</dt>
                <dd className="num">
                  {stats?.tiers.find((x) => x.tier === s.require_tier)
                    ?.done ?? "—"}
                </dd>
              </div>
              <div>
                <dt>{t.shopAvail}</dt>
                <dd className="num">
                  {(() => {
                    const done = stats?.tiers.find(
                      (x) => x.tier === s.require_tier,
                    )?.done;
                    return done === undefined
                      ? "—"
                      : Math.max(0, s.require_count - done);
                  })()}
                </dd>
              </div>
              <div>
                <dt>{t.shopCost}</dt>
                <dd className="num">{s.cost.toFixed(1)}</dd>
              </div>
              <div>
                <dt>{t.shopStock}</dt>
                <dd className="num">
                  {s.stock > 0
                    ? s.stock - s.stock_used
                    : dict.review.shopUnlimited}
                </dd>
              </div>
            </dl>
            <button
              type="button"
              className="brand-button"
              disabled={!s.available || busyId === s.id}
              title={
                !s.available
                  ? dict.review.taskShopSoon
                  : fmt(dict.review.taskShopCost, {
                      n: s.cost.toFixed(1),
                      magic: currency,
                    })
              }
              onClick={() => onBuy(s)}
            >
              {busyId === s.id
                ? dict.review.taskShopBuying
                : s.available
                  ? t.shopBuy
                  : dict.review.taskShopSoon}
            </button>
          </article>
        ))}
      </div>
    </section>
  );
}

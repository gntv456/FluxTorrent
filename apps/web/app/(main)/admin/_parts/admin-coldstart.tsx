"use client";

import { useI18n } from "@/i18n/client";
import type { Overview, StatsData } from "./admin-shared";
import { Badge } from "@/components/ui/badge";

/** PM-4：内容冷启动清单——把 playbook 06 的三步法产品化。
 *  只读取后台已有的 ov/stats，不新增请求；已完成项自动点亮。 */
export function AdminColdstart({
  ov,
  stats,
  onOpen,
}: {
  ov: Overview | null;
  stats: StatsData | null;
  onOpen: (tool: string) => void;
}) {
  const { dict } = useI18n();
  const u = dict.uxOpt;
  const tasks = [
    {
      key: "member",
      label: u.coldstartTaskMember,
      done: (ov?.users ?? 0) > 1,
      to: "users",
    },
    {
      key: "torrent",
      label: u.coldstartTaskTorrent,
      done: (ov?.torrents ?? 0) > 0,
      to: "torrents",
    },
    {
      key: "alive",
      label: u.coldstartTaskAlive,
      done: (ov?.health?.alive_rate ?? 0) > 0,
      to: "trackers",
    },
    {
      key: "review",
      label: u.coldstartTaskReview,
      done: (ov?.pending_reviews ?? 0) === 0,
      to: "reviews",
    },
    {
      key: "community",
      label: u.coldstartTaskCommunity,
      done: (stats?.comments ?? 0) > 0,
      to: "content",
    },
  ];
  const doneCount = tasks.filter((t) => t.done).length;

  return (
    <section className="ux-coldstart" aria-label={u.coldstartTitle}>
      <div className="ux-coldstart__head">
        <h2 className="ux-coldstart__title">
          {u.coldstartTitle} · {doneCount}/{tasks.length}
        </h2>
        <p className="ux-coldstart__hint">{u.coldstartHint}</p>
      </div>
      <div className="ux-coldstart__list">
        {tasks.map((t) => (
          <div
            key={t.key}
            className={
              t.done
                ? "ux-coldstart__item is-done"
                : "ux-coldstart__item is-todo"
            }
          >
            <Badge
              tone={t.done ? "success" : "warn"}
              size="sm"
            >
              {t.done ? "✓" : "○"}
            </Badge>
            <span className="ux-coldstart__label">{t.label}</span>
            {!t.done && (
              <button
                type="button"
                className="ux-coldstart__link"
                data-ux-event="coldstart_task_open"
                data-ux-label={t.key}
                onClick={() => onOpen(t.to)}
              >
                {u.coldstartOpen}
              </button>
            )}
          </div>
        ))}
      </div>
    </section>
  );
}

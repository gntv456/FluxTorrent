"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import {
  type TaskRow,
  type TaskOverview,
  TaskDashboard,
  TaskHistory,
} from "@/components/task-board-parts";
import {
  TaskRulesPanel,
  TaskTierPanel,
  TaskShopPanel,
} from "@/components/task-board-sections";
import { Tabs, TabPanel } from "@/components/ui/tabs";
import { toast } from "@/components/ui/toast";

// 任务系统（参考站 task.php 复刻）：TASK SYSTEM hero + 两组标签
// （规则｜我的任务记录、可领取任务｜任务商店）+ 01 最新动态 + 02 任务统计。
// 三个标签面板拆在 @/components/task-board-sections，数据契约/进度工具/
// 记录子面板拆在 @/components/task-board-parts（300 行门禁 + 按域分文件）。

export function TaskBoard({ sparkBalance }: { sparkBalance: number | null }) {
  const { dict, currency } = useI18n();
  const t = dict.tasks2;
  const [tasks, setTasks] = useState<TaskRow[] | null>(null);
  const [ov, setOv] = useState<TaskOverview | null>(null);
  const [busyId, setBusyId] = useState<number | null>(null);
  const [infoTab, setInfoTab] = useState("rules");
  const [boardTab, setBoardTab] = useState("tiers");

  const load = async () => {
    try {
      const [tl, o] = await Promise.all([
        api.get<TaskRow[]>("/api/v1/tasks"),
        api.get<TaskOverview>("/api/v1/tasks/overview"),
      ]);
      setTasks(tl);
      setOv(o);
    } catch {
      // P-314：任务列表失败此前 setTasks([]) ⇒ 演成「今天没任务可做」（签到/日常
      // 面板直接影响收益判断）。tasks 本就是 TaskRow[] | null，置 null + 全局 toast。
      setTasks(null);
      toast.error(dict.common.loadFailed);
    }
  };
  useEffect(() => {
    load();
  }, []);

  async function claim(id: number) {
    setBusyId(id);
    try {
      await api.post("/api/v1/tasks/claim", { task_id: id });
      // A1 反馈协议：领取结果走 toast（失败带 requestId 芯片）
      toast.success(t.claimOk);
      load();
    } catch (e) {
      toast.error(
        e instanceof ApiError ? e.message : dict.common.networkError,
        {
          requestId: e instanceof ApiError ? e.requestId : undefined,
        },
      );
    } finally {
      setBusyId(null);
    }
  }

  return (
    <div className="task-page">
      {/* Hero */}
      <section className="task-hero">
        <div>
          <span className="task-eyebrow">{t.heroEyebrow}</span>
          <h2>{t.title}</h2>
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

      {/* 标签组一：规则 ｜ 我的任务记录 */}
      <div className="task-tabblock">
        <Tabs
          idPrefix="taskinfo"
          label={t.title}
          value={infoTab}
          onChange={setInfoTab}
          items={[
            { key: "rules", label: t.rulesTitle },
            { key: "history", label: t.historyTitle },
          ]}
        />
        {infoTab === "rules" ? (
          <TabPanel idPrefix="taskinfo" tabKey="rules">
            <TaskRulesPanel />
          </TabPanel>
        ) : (
          <TabPanel idPrefix="taskinfo" tabKey="history">
            <TaskHistory ov={ov} />
          </TabPanel>
        )}
      </div>

      {/* 标签组二：可领取任务 ｜ 任务商店 */}
      <div className="task-tabblock">
        <Tabs
          idPrefix="taskboard"
          label={t.title}
          value={boardTab}
          onChange={setBoardTab}
          items={[
            { key: "tiers", label: t.tiersTitle },
            { key: "shop", label: t.shopTitle },
          ]}
        />
        {boardTab === "tiers" ? (
          <TabPanel idPrefix="taskboard" tabKey="tiers">
            <TaskTierPanel
              tasks={tasks ?? []}
              busyId={busyId}
              onClaim={claim}
            />
          </TabPanel>
        ) : (
          <TabPanel idPrefix="taskboard" tabKey="shop">
            <TaskShopPanel shop={ov?.shop ?? []} />
          </TabPanel>
        )}
      </div>

      {/* 01 最新动态 + 02 任务统计 */}
      <TaskDashboard ov={ov} />
    </div>
  );
}

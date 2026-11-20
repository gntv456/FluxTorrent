"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { clientUuid } from "@/lib/client-uuid";
import { fmt } from "@/i18n/config";
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
  // 领取任务扣报名费后 hero 余额此前不刷新（RSC prop 一次性）——本地镜像 + 重取
  const [bal, setBal] = useState(sparkBalance);
  useEffect(() => setBal(sparkBalance), [sparkBalance]);
  const [tasks, setTasks] = useState<TaskRow[] | null>(null);
  const [ov, setOv] = useState<TaskOverview | null>(null);
  const [busyId, setBusyId] = useState<number | null>(null);
  const [infoTab, setInfoTab] = useState("history");
  const [boardTab, setBoardTab] = useState("tiers");
  const [shopBusyId, setShopBusyId] = useState<number | null>(null);

  /** 兑换任务商店权益：退还门槛不足/售罄的文案按后端消息展示；
   *  成功后重取 overview 让库存、余额、券面板同步（幂等键保网络重试不双扣）。 */
  const buyShopItem = useCallback(
    async (item: TaskOverview["shop"][number]) => {
      setShopBusyId(item.id);
      try {
        const resp = await api.post<{ name: string; cost: number; replayed?: boolean }>(
          "/api/v1/tasks/shop/buy",
          {
            item_id: item.id,
            idempotency_key: `web-task-shop-${item.id}-${clientUuid()}`,
          },
        );
        toast.success(
          fmt(dict.review.taskShopBought, {
            name: resp.name,
            n: String(resp.cost),
            magic: currency,
          }),
        );
        // 刷新余额与库存（权威值）
        api.get<{ spark_balance: number }>("/api/v1/me/spark")
          .then((r) => setBal(r.spark_balance))
          .catch(() => {});
        load();
      } catch (e) {
        toast.error(apiErrorMessage(dict, e), {
          requestId: e instanceof ApiError ? e.requestId : undefined,
        });
      } finally {
        setShopBusyId(null);
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [dict, currency],
  );

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
      const fee = tasks?.find((x) => x.id === id)?.fee ?? 0;
      if (fee > 0) setBal((b) => (b === null ? null : b - fee));
      // 余额权威值重取一次（本地扣减只是即时反馈）
      api.get<{ spark_balance: number }>("/api/v1/me/spark")
        .then((r) => setBal(r.spark_balance))
        .catch(() => {});
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
            <strong className="num">{bal?.toFixed(1) ?? "—"}</strong>
          </div>
          <div>
            <span>{t.settleMode}</span>
            <strong>{t.settleLive}</strong>
          </div>
        </div>
      </section>

      {/* 标签组一：可领取任务 ｜ 任务商店（先于规则） */}
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
            <TaskShopPanel
          shop={ov?.shop ?? []}
          stats={ov?.stats}
          onBuy={(item) => void buyShopItem(item)}
          busyId={shopBusyId}
        />
          </TabPanel>
        )}
      </div>

      {/* 标签组二：规则 ｜ 我的任务记录（放在任务之后：先让人领任务） */}
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

      {/* 01 最新动态 + 02 任务统计 */}
      <TaskDashboard ov={ov} />
    </div>
  );
}

import { TaskBoard } from "@/components/task-board";
import { getDict } from "@/i18n/server";
import { requireModule } from "@/components/module-gate";
import { api } from "@/lib/api-client";

export const dynamic = "force-dynamic";

/** 任务系统（参考站 task.php 复刻）：五档任务卡 + 商店 + 动态 + 统计 + 记录 */
export default async function TasksPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("tasks");
  if (gate) return gate;

  const { dict } = await getDict();
  // 报名费 3~10 万，余额是领取决策的直接依据（评审 P0-3：此前恒传 null
  // 显示「—」）。取档失败回落 null——面板显示「—」而非阻塞整页。
  const me = await api
    .get<{ spark_balance?: number }>("/api/v1/me/overview")
    .catch(() => null);
  return (
    <div className="flex flex-col gap-4">
      <h1 className="sr-only">{dict.tasks.title}</h1>
      <TaskBoard sparkBalance={me?.spark_balance ?? null} />
    </div>
  );
}

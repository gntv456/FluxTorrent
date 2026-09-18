import { TaskBoard } from "@/components/task-board";
import { getDict } from "@/i18n/server";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 任务系统（参考站 task.php 复刻）：五档任务卡 + 商店 + 动态 + 统计 + 记录 */
export default async function TasksPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("tasks");
  if (gate) return gate;

  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="sr-only">{dict.tasks.title}</h1>
      <TaskBoard sparkBalance={null} />
    </div>
  );
}

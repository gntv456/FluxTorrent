import { TaskBoard } from "@/components/task-board";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** 任务系统（包子站 task.php 复刻）：五档任务卡 + 商店 + 动态 + 统计 + 记录 */
export default async function TasksPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="sr-only">{dict.tasks.title}</h1>
      <TaskBoard sparkBalance={null} />
    </div>
  );
}

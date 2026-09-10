import { getTasks } from "@/lib/data";
import { getDict } from "@/i18n/server";
import { TaskList } from "@/components/task-actions";

export const dynamic = "force-dynamic";

/** 任务系统（包子站 task.php 同款）：完成做种/上传指标解锁奖励 */
export default async function TasksPage() {
  const { dict } = await getDict();
  const tasks = await getTasks();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.tasks.title}</h1>
        <span className="text-sm text-sub">{dict.tasks.subtitle}</span>
      </div>
      <TaskList empty={dict.tasks.empty} />
      <p className="text-xs text-sub">
        {dict.tasks.current}: {tasks.length}
      </p>
    </div>
  );
}

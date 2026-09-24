import { TeamPanel } from "@/components/team-panel";
import { getDict } from "@/i18n/server";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 保种协作（0102 社交层）：组队把濒危资源保回来，奖励按各自贡献分摊。
 * 与 /resurrections（单人认领死种）互补：这里面向多人分摊的协作保种。 */
export default async function TeamsPage() {
  const gate = await requireModule("social");
  if (gate) return gate;
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.teams.title}</h1>
      <TeamPanel />
    </div>
  );
}

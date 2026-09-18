import { EndangeredPanel } from "@/components/endangered-panel";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** 濒危预警雷达（0102 社交层）——
 * 与 /resurrections（复活已死资源）互补：这里盯的是「只剩个别做种者、还没死」的资源。 */
export default async function EndangeredPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.endangered.title}</h1>
      <EndangeredPanel />
    </div>
  );
}

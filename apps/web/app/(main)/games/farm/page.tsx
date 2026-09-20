import { api } from "@/lib/api-client";
import { requireModule } from "@/components/module-gate";
import FarmPage, { type FarmData, type MeData } from "./_inner";

export default async function Page() {
  // 农场是独立模块（站长可只关农场不关赌局），门控沿用 farm 键而非 games
  const gate = await requireModule("farm");
  if (gate) return gate;
  // 服务端预取：首屏即有田地与行情（客户端仍会自行刷新）
  const [initialFarm, initialMe] = await Promise.all([
    api.get<FarmData>("/api/v1/farm").catch(() => null),
    api
      .get<{ me?: MeData }>("/api/v1/games")
      .then((r) => r.me ?? null)
      .catch(() => null),
  ]);
  return <FarmPage initialFarm={initialFarm} initialMe={initialMe} />;
}

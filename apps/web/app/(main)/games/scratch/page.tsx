import { api } from "@/lib/api-client";
import { requireModule } from "@/components/module-gate";
import ScratchPage, { type Overview } from "./_inner";

export default async function Page() {
  const gate = await requireModule("games");
  if (gate) return gate;
  // 服务端预取规则/奖池/余额：首屏即有内容（客户端仍会自行刷新）
  const initialOver = await api.get<Overview>("/api/v1/games").catch(() => null);
  return <ScratchPage initialOver={initialOver} />;
}

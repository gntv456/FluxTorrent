import { api } from "@/lib/api-client";
import { requireModule } from "@/components/module-gate";
import LeaderboardPage, { type BoardData } from "./_inner";

/** 周榜页：服务端取数 + 客户端切换四张榜 */
export default async function Page() {
  const gate = await requireModule("games");
  if (gate) return gate;
  const initial = await api
    .get<BoardData>("/api/v1/games/arcade/leaderboard")
    .catch(() => null);
  return <LeaderboardPage initial={initial} />;
}

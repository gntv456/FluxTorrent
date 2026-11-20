import { api } from "@/lib/api-client";
import { requireModule } from "@/components/module-gate";
import { ErrorPanel } from "@/components/ui/error-panel";
import { normalizeError } from "@/lib/render-error";
import LeaderboardPage, { type BoardData } from "./_inner";

/** 周榜页：服务端取数 + 客户端切换四张榜 */
export default async function Page() {
  const gate = await requireModule("games");
  if (gate) return gate;
  let initial: BoardData;
  try {
    initial = await api.get<BoardData>("/api/v1/games/arcade/leaderboard");
  } catch (e) {
    return (
      <ErrorPanel
        error={normalizeError(e)}
        retryHref="/games/leaderboard"
      />
    );
  }
  return <LeaderboardPage initial={initial} />;
}

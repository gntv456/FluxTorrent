import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { PageHeader } from "@/components/ui/page-header";
import { ReadState } from "@/components/ui/read-state";
import { ContractSignBoard } from "./_parts/sign-board";
import { ContractHistory } from "./_parts/history";

export const dynamic = "force-dynamic";

interface Tier {
  kind: string;
  label: string;
  target: number;
  reward: number;
  penalty: number;
  sort: number;
}

interface TiersResp {
  enabled: boolean;
  eligible: boolean;
  progress: null | {
    kind: string;
    target: number;
    current: number;
    recorded: number;
  };
  tiers: Tier[];
}

interface MineRow {
  id: number;
  kind: string;
  month: string;
  target: number;
  achieved: number;
  status: string;
  reward: number;
  penalty: number;
  auto_renew: boolean;
  missed_streak: number;
}

/** 月度做种契约（0424 后端 / 0498 前端收口）：档位签约 + 当月进度 + 历史。
 *  此前后端三端点完整但前端零调用——用户无法签约、不知存在（产品体检
 *  2026-11-18 A1「有后端无前端」头号断档）。本页一次收口三口。 */
export default async function ContractsPage() {
  const { dict } = await getDict();
  const t = dict.contracts;
  let tiers: TiersResp | null = null;
  let mine: MineRow[] | null = null;
  try {
    [tiers, mine] = await Promise.all([
      api.get<TiersResp>("/api/v1/contracts/tiers"),
      api.get<MineRow[]>("/api/v1/contracts/mine"),
    ]);
  } catch {
    tiers = null;
    mine = null;
  }
  return (
    <div className="flex flex-col gap-4">
      <PageHeader eyebrow="Contract" title={t.title} sub={t.sub} />
      <ReadState failed={tiers === null} dict={dict} emptyTitle={t.empty}>
        {tiers && (
          <ContractSignBoard tiers={tiers} dict={dict} />
        )}
        {mine && <ContractHistory rows={mine} dict={dict} />}
      </ReadState>
    </div>
  );
}

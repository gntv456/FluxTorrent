import Link from "next/link";
import { api } from "@/lib/api-client";
import { requireModule } from "@/components/module-gate";
import { getDict } from "@/i18n/server";
import { FishingArcade } from "@/components/game/fishing-arcade";
import { CHIP_POND_BADGE } from "@/lib/ui-classes";

/**
 * 完整渔场：甜梦钓场（六类装备 / 图鉴 / 每日订单 / 好友赠饵 / 赛季排行）。
 *
 * 与 /games/fishing 的「押注式经典玩法」并存：
 * - 经典玩法走服务端判定的 round/cast/reel，动的是账号魔力；
 * - 这里是把设计稿落地的完整养成玩法，存档按登录账号保存在服务端
 *   （`arcade_fishing_pond`，写接口事务内锁行）；场内不使用独立软币，
 *   直接以站点魔力 `users.spark_balance` 计价：买装备、扩容、补体力即扣站点魔力。
 *   **钓场是魔力的回收口（2026-11-24 口径）**：渔获 / 订单 / 签到 / 成就 /
 *   赛季名次一律不发魔力（只计图鉴估值），`mirror_magic` 只扣不加。
 */
export default async function FishingPondPage() {
  const gate = await requireModule("games");
  if (gate) return gate;

  const [{ dict }, me] = await Promise.all([
    getDict(),
    api
      .get<{ username?: string }>("/api/v1/me/overview")
      .catch(() => null),
  ]);
  const tf = dict.games.fishing;

  return (
    <div className="mx-auto w-full max-w-[1180px] px-3 pb-24 sm:px-4 md:pb-5">
      <div className="mb-3 flex flex-wrap items-end justify-between gap-2">
        <div>
          <div className="flex items-center gap-2 text-xs text-sub">
            <Link
              href="/games/fishing"
              className="underline-offset-2 hover:underline"
            >
              {tf.pondBack}
            </Link>
            <span aria-hidden>/</span>
            <span>{tf.pondCrumb}</span>
          </div>
          <h1 className="mt-1 flex items-center gap-2 text-lg font-semibold">
            {tf.pondTitle}
            <span className={CHIP_POND_BADGE}>{tf.pondBadge}</span>
          </h1>
          <p className="mt-1 text-xs text-sub">{tf.pondSub}</p>
        </div>
        <div className="flex items-center gap-3 text-xs text-sub">
          <a
            href="/arcade/fishing/index.html"
            target="_blank"
            rel="noreferrer"
            className="underline-offset-2 hover:underline"
          >
            {tf.pondOpenAlone}
          </a>
        </div>
      </div>

      <FishingArcade ns={me?.username ?? "guest"} />

      <p className="mt-2 hidden text-[11px] leading-relaxed text-sub md:block">
        {tf.pondFootNote}
        <Link href="/games/fishing" className="mx-1 underline">
          {tf.pondFootAfter}
        </Link>
        {tf.pondFootTail}
      </p>
    </div>
  );
}

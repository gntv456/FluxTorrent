import { getFriends, getPolls, getOffers } from "@/lib/data";
import { PANEL_MD } from "@/lib/ui-classes";
import { getDict } from "@/i18n/server";
import { PollBox, OfferList } from "@/components/community-extra";
import { FriendsActions } from "@/components/friends-actions";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 社交名单（参考站 friends.php 同款）：好友 + 黑名单 + 操作入口（加友/接受/拉黑） */
export default async function FriendsPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("friends");
  if (gate) return gate;

  const { dict } = await getDict();
  const friends = await getFriends();
  const friendRows = friends.filter((f) => f.list === "friend");
  const blackRows = friends.filter((f) => f.list === "black");
  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col gap-4">
        <div className="pghd">
          <div>
            <div className="pg-eyebrow">Friends</div>
            <h1 className="font-display text-2xl">{dict.friends.title}</h1>
          </div>
          <span className="sub">{dict.friends.subtitle}</span>
        </div>
        {/* 操作面板：加好友 / 接受申请 / 拉黑（此前只读列表，后端能力无入口） */}
        <section className={PANEL_MD}>
          <FriendsActions />
        </section>
        <section className={PANEL_MD}>
          <h2 className="font-bold">{dict.friends.friends}</h2>
          {friendRows.length === 0 ? (
            <p className="py-4 text-center text-sub">
              {dict.friends.emptyFriends}
            </p>
          ) : (
            <ul className="mt-2 flex flex-wrap gap-2">
              {friendRows.map((f) => (
                <li key={f.username}>
                  <span className="pill">
                    {f.username}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </section>
        <section className={PANEL_MD}>
          <h2 className="font-bold">{dict.friends.blocklist}</h2>
          {blackRows.length === 0 ? (
            <p className="py-4 text-center text-sub">
              {dict.friends.emptyBlack}
            </p>
          ) : (
            <ul className="mt-2 flex flex-wrap gap-2">
              {blackRows.map((f) => (
                <li key={f.username}>
                  <span className="pill">
                    {f.username}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>

      <div className="flex flex-col gap-4">
        <h2 className="font-display text-xl">{dict.polls.title}</h2>
        <PollBox empty={dict.polls.empty} />
      </div>

      <div className="flex flex-col gap-4">
        <h2 className="font-display text-xl">{dict.offers.title}</h2>
        <OfferList empty={dict.offers.empty} offers={await getOffers()} />
      </div>
    </div>
  );
}

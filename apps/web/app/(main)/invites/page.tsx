import { getDict } from "@/i18n/server";
import { InviteManager } from "@/components/invite-manager";

export const dynamic = "force-dynamic";

/** 邀请管理（包子站 invite.php 同款）：LV3+ 每周 2 枚 */
export default async function InvitesPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.invites.title}</h1>
        <span className="text-sm text-sub">{dict.invites.subtitle}</span>
      </div>
      <InviteManager
        empty={dict.invites.empty}
        issueLabel={dict.invites.issue}
        needClass={dict.invites.needClass}
      />
    </div>
  );
}

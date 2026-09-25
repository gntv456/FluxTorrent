import { getDict } from "@/i18n/server";
import { InviteManager } from "@/components/invite-manager";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 邀请管理（NP invite.php 口径）：配额概览 + 生成/兑换 + 邮件发送 + 状态列表 */
export default async function InvitesPage() {
  // U1 模块页守卫：invites 关闭时渲染统一空态（四审 L5 P1 补挂键）
  const gate = await requireModule("invites");
  if (gate) return gate;

  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.invites.title}</h1>
        <span className="text-sm text-sub">{dict.invites.subtitle}</span>
      </div>
      <InviteManager />
    </div>
  );
}

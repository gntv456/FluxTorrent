import { getDict } from "@/i18n/server";
import { PageHeader } from "@/components/ui/page-header";
import { InviteManager } from "@/components/invite-manager";
import { requireModule } from "@/components/module-gate";
import { getSiteProfile } from "@/lib/site-profile";

export const dynamic = "force-dynamic";

/** 邀请管理（NP invite.php 口径）：配额概览 + 生成/兑换 + 邮件发送 + 状态列表 */
export default async function InvitesPage() {
  // U1 模块页守卫：invites 关闭时渲染统一空态（四审 L5 P1 补挂键）
  const gate = await requireModule("invites");
  if (gate) return gate;

  const { dict } = await getDict();
  const profile = await getSiteProfile();
  return (
    <div className="flex flex-col gap-4">
      {/* 2026-10-17 对齐样图集 § tplInvite：原先这里是裸 h1 + span，改成
          全站统一的 PageHeader（白卡外壳 + 眉标 + 副标题）。眉标沿用项目
          既有惯例的西文短词（同 Bank/Donate/Dressup），不照搬样图的中文
          面包屑——.pg-eyebrow 是 uppercase + 0.16em 字距的西文样式，
          塞中文会散开，而那属于「新增体系」。 */}
      <PageHeader
        eyebrow="Invites"
        title={dict.invites.title}
        sub={dict.invites.subtitle}
        aside={<span className="sticker">{profile.brand}</span>}
      />
      <InviteManager />
    </div>
  );
}

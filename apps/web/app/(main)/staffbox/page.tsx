import { redirect } from "next/navigation";
import { StaffBox } from "@/components/staff-box";
import { api } from "@/lib/api-client";
import { isStaffClass } from "@/lib/domain/user-class";
import { getDict } from "@/i18n/server";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 管理组信箱（staffbox.php 复刻）：与管理组的往来短讯 */
export default async function StaffBoxPage() {
  const gate = await requireModule("messages");
  if (gate) return gate;
  // /staffmessages 与 /stafftickets 要 staff.message（只授 class>=90），
  // 而侧栏「管理组信箱」对全体会员可见 ⇒ 会员进来默认落「咨询工作台」
  // 先吃一张 403 错误面板。会员的入口是 /contactstaff（写信 + 我的工单）；
  // /me 取不到时放行，别把 staff 也一起挡在门外。
  const me = await api
    .get<{ class_id?: number }>("/api/v1/me")
    .catch(() => null);
  if (me && !isStaffClass(me.class_id)) redirect("/contactstaff");
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.staffbox.title}</h1>
      <StaffBox />
    </div>
  );
}

import { redirect } from "next/navigation";
import { api } from "@/lib/api-client";
import { getSiteProfile } from "@/lib/site-profile";
import { isStaffClass } from "@/lib/domain/user-class";

/** 维护模式判定（0462）：维护页/员工横幅共用的取数。
 *  - profile 取不到（API 挂了）一律按「未维护」放行——维护开关读挂把
 *    staff 也关在维护页外，比晚 30s 生效糟得多（与 API 闸门 fail-open 同口径）；
 *  - 未登录（无 me）落维护页（那里保留登录出口）；
 *  - staff（class_id>=90，与后端 require_staff 同线）穿透 + 看横幅。
 *  返回值：null=不在维护；{staff:boolean}=在维护。 */
export async function maintenanceGate(): Promise<{
  staff: boolean;
} | null> {
  const profile = await getSiteProfile().catch(() => null);
  if (!profile?.maintenance?.on) return null;
  // /me 在 API 维护闸门白名单里（同 auth_infra 强制改密三口的思路：
  // 被闸门拦死的用户连「自己是谁」都读不到，就永远出不去维护态）
  const me = await api
    .get<{ class_id?: number }>("/api/v1/me")
    .catch(() => null);
  return { staff: isStaffClass(me?.class_id) };
}

/** (main) 布局用的重定向捷径：非 staff 一律 302 维护页。
 *  在维护 & staff → 返回 true（布局渲染横幅），调用方自行处理。 */
export async function requireNotMaintaining(): Promise<boolean> {
  const gate = await maintenanceGate();
  if (!gate) return false;
  if (!gate.staff) redirect("/maintenance");
  return true;
}

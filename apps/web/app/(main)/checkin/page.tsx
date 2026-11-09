import { getDict } from "@/i18n/server";
import { PageHeader } from "@/components/ui/page-header";
import { CheckinPage } from "@/components/checkin-page";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 签到页（桌面完整版）：主动作 + 连签/累计/补签卡 + 可翻月万年历。 */
export default async function CheckinRoute() {
  const gate = await requireModule("attendance");
  if (gate) return gate;

  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow="Check-in"
        title={dict.home2.attendanceTitle}
        sub={dict.home2.attendanceTitle}
      />
      <CheckinPage />
    </div>
  );
}

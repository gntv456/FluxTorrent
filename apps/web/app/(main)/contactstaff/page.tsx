import { ContactStaff } from "@/components/contact-staff";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** PM 管理组（参考站 contactstaff.php 复刻） */
export default async function ContactStaffPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="sr-only">{dict.contactstaff.title}</h1>
      <ContactStaff />
    </div>
  );
}

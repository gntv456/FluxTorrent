import { redirect } from "next/navigation";

/** 旧入口 /farm 统一重定向到娱乐屋农场专注页 */
export default function FarmLegacyPage() {
  redirect("/games/farm");
}

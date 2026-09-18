import { requireModule } from "@/components/module-gate";
import JixiaoPage from "./_inner";

export default async function JixiaoPageWrapper() {
  const gate = await requireModule("jixiao");
  if (gate) return gate;
  return <JixiaoPage />;
}

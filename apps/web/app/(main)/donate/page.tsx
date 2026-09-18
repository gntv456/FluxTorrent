import { requireModule } from "@/components/module-gate";
import DonatePage from "./_inner";

export default async function DonatePageWrapper() {
  const gate = await requireModule("magic_pool");
  if (gate) return gate;
  return <DonatePage />;
}

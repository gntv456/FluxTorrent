import { requireModule } from "@/components/module-gate";
import OffersPage from "./_inner";

export default async function OffersPageWrapper() {
  const gate = await requireModule("offers");
  if (gate) return gate;
  return <OffersPage />;
}

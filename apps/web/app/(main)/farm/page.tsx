import { requireModule } from "@/components/module-gate";
import FarmPage from "./_inner";

export default async function FarmPageWrapper() {
  const gate = await requireModule("farm");
  if (gate) return gate;
  return <FarmPage />;
}

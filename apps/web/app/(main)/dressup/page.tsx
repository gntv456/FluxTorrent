import { requireModule } from "@/components/module-gate";
import DressupPage from "./_inner";

export default async function DressupPageWrapper() {
  const gate = await requireModule("dressup");
  if (gate) return gate;
  return <DressupPage />;
}

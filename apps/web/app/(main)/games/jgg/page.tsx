import { requireModule } from "@/components/module-gate";
import JggPage from "./_inner";

export default async function Page() {
  const gate = await requireModule("games");
  if (gate) return gate;
  return <JggPage />;
}

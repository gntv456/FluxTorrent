import { api } from "@/lib/api-client";
import { requireModule } from "@/components/module-gate";
import LotteryPage from "./_inner";

export default async function Page() {
  const gate = await requireModule("games");
  if (gate) return gate;
  return <LotteryPage />;
}

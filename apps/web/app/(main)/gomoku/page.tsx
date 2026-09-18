import { requireModule } from "@/components/module-gate";
import GomokuPage from "./_inner";

export default async function GomokuPageWrapper() {
  const gate = await requireModule("gomoku");
  if (gate) return gate;
  return <GomokuPage />;
}

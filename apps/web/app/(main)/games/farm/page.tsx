import { requireModule } from "@/components/module-gate";
import FarmPage from "./_inner";

export default async function Page() {
  // 农场是独立模块（站长可只关农场不关赌局），门控沿用 farm 键而非 games
  const gate = await requireModule("farm");
  if (gate) return gate;
  return <FarmPage />;
}

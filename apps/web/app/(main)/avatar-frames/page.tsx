import { requireModule } from "@/components/module-gate";
import AvatarFramesPage from "./_inner";

export default async function AvatarFramesPageWrapper() {
  const gate = await requireModule("dressup");
  if (gate) return gate;
  return <AvatarFramesPage />;
}

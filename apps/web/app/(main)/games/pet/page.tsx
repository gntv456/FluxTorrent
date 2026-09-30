import { api } from "@/lib/api-client";
import { requireModule } from "@/components/module-gate";
import PetPage, { type PetStatus } from "./_inner";

export default async function Page() {
  const gate = await requireModule("games");
  if (gate) return gate;
  const initial = await api
    .get<PetStatus>("/api/v1/games/pet")
    .catch(() => null);
  return <PetPage initial={initial} />;
}

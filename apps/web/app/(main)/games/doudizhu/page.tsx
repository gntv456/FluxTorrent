import { api } from "@/lib/api-client";
import { requireModule } from "@/components/module-gate";
import DoudizhuRoom from "./_room";
import type { Overview } from "./_parts/room-types";

export default async function Page() {
  const gate = await requireModule("games");
  if (gate) return gate;
  const initialOver = await api
    .get<Overview>("/api/v1/games")
    .catch(() => null);
  return <DoudizhuRoom initialOver={initialOver} />;
}

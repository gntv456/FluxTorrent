/**
 * 宠物种类与成长线（纯数据，服务端/客户端共用）。
 *
 * 四条线都是 10 级、蛋起手、各有 apex —— 种类只改外观，不改任何数值；
 * 数值差异化就是 pay-to-win 的第一块砖。与后端 `pet_custom.rs::SPECIES`
 * 白名单一一对应，加种类两端同步。
 */

export const PET_SPECIES = ["slime", "cat", "bunny", "drake"] as const;

export type PetSpecies = (typeof PET_SPECIES)[number];

/** i18n 键（dict.games.pet.sp*） */
export const PET_SPECIES_KEY: Record<PetSpecies, string> = {
  slime: "spSlime",
  cat: "spCat",
  bunny: "spBunny",
  drake: "spDrake",
};

export const PET_LINES: Record<PetSpecies, string[]> = {
  slime: ["🥚", "🐣", "🐥", "🐤", "🐔", "🦆", "🦢", "🦅", "🦉", "🐲"],
  cat: ["🥚", "🐣", "🐱", "🐱", "🐈", "🐈", "🐈‍⬛", "🐆", "🐅", "🦁"],
  bunny: ["🥚", "🐣", "🐰", "🐰", "🐹", "🐹", "🐇", "🐇", "🐇", "🐰"],
  drake: ["🥚", "🐣", "🦎", "🦎", "🐊", "🦕", "🦖", "🐍", "🐉", "🐲"],
};

export function petLineOf(species: string): string[] {
  return PET_LINES[(PET_SPECIES as readonly string[]).includes(species)
    ? (species as PetSpecies)
    : "slime"];
}

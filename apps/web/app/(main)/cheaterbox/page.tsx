import { CheaterBox } from "@/components/cheater-box";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** 作弊者信箱（好学站 cheaterbox.php 复刻） */
export default async function CheaterBoxPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.cheaterbox.title}</h1>
      <CheaterBox />
    </div>
  );
}

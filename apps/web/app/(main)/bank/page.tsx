import { getDict } from "@/i18n/server";
import { BankCard } from "@/components/bank-actions";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 银行系统（火花银行对齐）：活期复利 + 定期分档 + 贷款 */
export default async function BankPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("bank");
  if (gate) return gate;

  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">Bank</div>
          <h1 className="font-display text-2xl">{dict.bank.title}</h1>
        </div>
        <span className="sub">{dict.bank.subtitle}</span>
      </div>
      <BankCard />
    </div>
  );
}

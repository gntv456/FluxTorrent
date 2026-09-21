import { getDict } from "@/i18n/server";
import { BankCard } from "@/components/bank-actions";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 银行系统（火花银行对齐）：活期复利 + 定期分档 + 贷款 */
export default async function BankPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("bank");
  if (gate) return gate;

  const { dict, currency } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.bank.title}</h1>
        <span className="text-sm text-sub">{dict.bank.subtitle}</span>
      </div>
      <BankCard
        loginToView={dict.my.loginToView.replace("{magic}", currency)}
      />
    </div>
  );
}

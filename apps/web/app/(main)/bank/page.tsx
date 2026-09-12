import { getDict } from "@/i18n/server";
import { BankCard } from "@/components/bank-actions";

export const dynamic = "force-dynamic";

/** 银行系统（火花银行对齐）：活期复利 + 定期分档 + 贷款 */
export default async function BankPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.bank.title}</h1>
        <span className="text-sm text-sub">{dict.bank.subtitle}</span>
      </div>
      <BankCard loginToView={dict.my.loginToView} />
    </div>
  );
}

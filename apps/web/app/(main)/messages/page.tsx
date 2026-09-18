import { MessageCenter } from "@/components/message-center";
import { getDict } from "@/i18n/server";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 站内消息中心（M17，旧站 messages.php）：收件/已发/写信 */
export default async function MessagesPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("messages");
  if (gate) return gate;

  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.messages.title}</h1>
      <MessageCenter />
    </div>
  );
}

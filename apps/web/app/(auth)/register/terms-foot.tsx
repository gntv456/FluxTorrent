"use client";

/** 注册页·条款脚注（0423 P0-1 从 register/page.tsx 拆出，300 行门禁）：
 *  注册条款从死文字改为带 /rules 链接——出厂配置下新用户全程可能没点开
 *  过任何规则页，「同意」至少给一个规则入口。 */

import Link from "next/link";
import { useI18n } from "@/i18n/client";

export function TermsFoot() {
  const { dict } = useI18n();
  return (
    <>
      {dict.register.termsNotePre}
      <Link href="/rules" className="underline">
        {dict.register.termsNoteRules}
      </Link>
      {dict.register.termsNotePrivacy}
    </>
  );
}

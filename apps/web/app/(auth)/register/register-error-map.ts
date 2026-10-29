"use client";

import { ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { fieldErrorsFrom } from "@/components/ui/form-field";

/** W-18：错误码→字段映射（2005 邀请码 / 2007 用户名；1002 走信封）。
 *  从 register/page.tsx 按域拆出（300 行门禁）。 */

const CODE_TO_FIELD: Record<number, string> = {
  2005: "invite_code", // 邀请码无效或已过期
  2006: "invite_code", // 邀请码已被使用
  2007: "username", // 用户名已被占用
  1002: "", // 多字段信封，fieldErrorsFrom 解析
};

/** W-18 错误路径：后端错 → 字段级 map。优先 1002 多字段信封
 *  （fieldErrorsFrom），单错误码按 CODE_TO_FIELD 归字段；归不了的
 *  落 form 键由 FieldErrors 顶部汇总渲染。 */
export function mapRegisterError(
  dict: ReturnType<typeof useI18n>["dict"],
  err: unknown,
): Record<string, string[]> {
  const fromEnvelope = fieldErrorsFrom(err);
  if (fromEnvelope) {
    return Object.fromEntries(
      Object.entries(fromEnvelope).map(([f, v]) => [
        f,
        Array.isArray(v) ? v : [v],
      ]),
    );
  }
  if (err instanceof ApiError) {
    // P-568：1002 的 message 是「参数校验失败: 具体判据」，取正文，
    // 别用字典标签把「邮箱被禁用」这类可操作的正确原因盖掉。
    const msg = apiErrorMessage(dict, err);
    const field = CODE_TO_FIELD[err.code];
    return field ? { [field]: [msg] } : { form: [msg] };
  }
  return { form: [dict.common.networkError] };
}


/** ApiError（从 lib/api-client.ts 按域拆出，300 行门禁）。
 *  api-client 顶部 `export { ApiError } from "./api-error"` 再导出——
 *  全站 378 处 `from "@/lib/api-client"` 的既有 import 不动。 */

export class ApiError extends Error {
  constructor(
    public readonly code: number,
    message: string,
    /** 信封 data：站点设定的字段级校验错误等结构化的错误载荷 */
    public readonly data?: unknown,
    /** HTTP 状态码：业务 code 分不出「不存在」与「暂时读不到」（P-35b） */
    public readonly status?: number,
    /** 信封 request_id（DS-PLAN-01 §0.0 判据 5「失败可诊断」）：
     *  与响应头/日志同源，ErrorPanel 展示 + 一键复制，用户报障自带线索 */
    public readonly requestId?: string,
  ) {
    super(message);
  }

  /** 字段级校验错误（DS-PLAN-01 W-14 表单协议）：后端 1002 信封的
   *  data 形如 [{field, error}]（settings 与 auth/upload 校验同口径）。
   *  本 getter 把它解成 Record<field, msgs[]>；非该形态返回 undefined，
   *  调用方据此决定走字段内联（FieldErrors/FormField）还是表单级提示。
   *  与 form-field.tsx 的 fieldErrorsFrom() 共用口径，信封形态以
   *  apps/api/src/errors.rs DomainError::FieldErrors 序列化为准。 */
  get fields(): Record<string, string[]> | undefined {
    if (!Array.isArray(this.data)) return undefined;
    const out: Record<string, string[]> = {};
    for (const item of this.data as { field?: unknown; error?: unknown }[]) {
      if (
        typeof item?.field !== "string" ||
        typeof item?.error !== "string"
      )
        continue;
      (out[item.field] ??= []).push(item.error);
    }
    return Object.keys(out).length > 0 ? out : undefined;
  }
}

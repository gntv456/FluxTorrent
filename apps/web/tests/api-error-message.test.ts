import { describe, it, expect } from "vitest";

import { apiErrorMessage } from "@/i18n/client";
import { ApiError } from "@/lib/api-client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-305：把 P-117 的「取具体判据」从 1002 一般化到所有多原因码。
 *
 * 后端在 envelope.message 里把字典标签拼在最前面再挂原因
 * （「参数校验失败: 还款金额必须为正」「种子文件无效: 种子未标记私有…」）。
 * 旧 helper 只给 1002 开特权，其余码一律拿字典那句盖掉；而站里 43 处内联
 * `dict.errors[code] ?? message` 连这个特权都绕过了。
 *
 * 四条一起跑才算把口径钉住：两类要露原因、两类要保留字典文案。
 */

type Dict = Parameters<typeof apiErrorMessage>[0];
const dict = zhCN as unknown as Dict;

describe("apiErrorMessage（P-305）", () => {
  it("1002：具体判据露出（P-117 的既有行为不变）", () => {
    const e = new ApiError(1002, "参数校验失败: 还款金额必须为正", null);
    expect(apiErrorMessage(dict, e)).toEqual("还款金额必须为正");
  });

  it("3003：多原因共用码同样露出原因（本批新增）", () => {
    const e = new ApiError(
      3003,
      "种子文件无效: 种子未标记私有：.torrent 的 info.private 必须为 1",
      null,
    );
    expect(apiErrorMessage(dict, e)).toContain("info.private");
    expect(apiErrorMessage(dict, e)).not.toEqual(zhCN.errors[3003]);
  });

  it("2004：message 不以字典标签开头时保留字典文案（登录那句是刻意软化）", () => {
    const e = new ApiError(2004, "用户名或密码错误", null);
    expect(apiErrorMessage(dict, e)).toEqual(zhCN.errors[2004]);
  });

  it("只有标签没有正文时不返回空串（退回粗标签）", () => {
    const e = new ApiError(1004, "资源不存在", null);
    expect(apiErrorMessage(dict, e)).toEqual("资源不存在");
  });

  it("裸信封对象（手搓 fetch 的站点）与 ApiError 同一口径", () => {
    const env = {
      code: 3003,
      message: "种子文件无效: 文件清单为空",
      data: null,
    };
    expect(apiErrorMessage(dict, env)).toEqual("文件清单为空");
  });

  it("非 ApiError 的普通异常仍走原路径（没被信封分支吃掉）", () => {
    expect(apiErrorMessage(dict, new Error("网络断了"))).toEqual("网络断了");
  });
});

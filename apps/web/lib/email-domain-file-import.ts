/** 域名库文件导入（multipart）：大清单不再受 2MiB JSON 请求体墙约束。
 *
 * JSON 口的批数上限只能由墙反推（IMPORT_MAX = 8176 条，P-638），整张
 * 公共清单一次灌不进；本口流式读 multipart，代价是解析上限由服务端
 * 自数、且中途失败时前面几批已提交——服务端错误文案会点名「已导入
 * 前 N 条」，这里原样交给界面，不吞成通用失败。
 */

import { ApiError, rawFetchHelpers } from "@/lib/api-client";

/** 与服务端 email_domains_file.rs 成功信封同形：skipped = 库里已有，
 *  duplicates = 文件内重复。 */
export type FileImportResp = {
  file: string;
  added: number;
  skipped: number;
  duplicates: number;
  invalid: number;
  total: number;
  batches: number;
  rejected: number;
};

export async function importEmailDomainsFile(
  file: File,
  fallback: string,
): Promise<FileImportResp> {
  const form = new FormData();
  form.append("file", file);
  const lang = rawFetchHelpers.lang();
  // 凭证由 HttpOnly flux_token cookie 自动携带（同源 rewrites 转发）；
  // 写方法必须带 CSRF 双提交头，否则 csrf_mw 先回 403/2003（P-500）。
  const csrf = await rawFetchHelpers.csrf("POST");
  const res = await fetch(
    rawFetchHelpers.base() + "/api/v1/admin/email-domains/import-file",
    {
      method: "POST",
      headers: {
        ...(lang ? { "Accept-Language": lang } : {}),
        ...csrf,
      },
      body: form,
    },
  );
  const body = (await res.json()) as {
    code: number;
    message?: string;
    data?: FileImportResp;
  };
  if (body.code !== 0 || !body.data) {
    throw new ApiError(body.code, body.message ?? fallback);
  }
  return body.data;
}
import {
  ApiError,
  acceptLanguage,
  baseUrl,
  csrfHeaderAsync,
  netMsg,
} from "@/lib/api-client";
import type { ApiEnvelope } from "@fluxtorrent/domain-types";

/** 失败响应归一化（批二百零三）：JSON 口与 blob 口共用一把尺。
 *
 *  后端把「该怎么办」写在信封 message 的「字典标签: 判据」尾巴上（形状即
 *  i18n/client.ts 的 apiErrorMessage / P-305 协议）。现网实测
 *  `GET /api/v1/torrents/49417/download` →
 *  `400 {"code":1002,"message":"参数校验失败: 种子文件缺失（原始 .torrent
 *  未存档），请联系管理组补档","request_id":…}`；同口还有「您的下载权限已被
 *  暂停，请联系管理组」（charge.rs:44）与火花不足 4001/409。
 *
 *  blob 口旧写法只看 HTTP 状态码造一句兜底文案 ⇒ 这句中文在 fetch 层就被丢掉，
 *  管理组没补档 / 下载权被暂停 / 余额不足三种完全不同的处境在界面上长一个样，
 *  会员只能盲重试或报障。信封读不出（网关 HTML、空体、截断）才回落兜底。 */
export async function toApiError(
  res: Response,
  fallback: string,
): Promise<ApiError> {
  const ctype = res.headers.get("content-type") ?? "";
  if (ctype.includes("application/json")) {
    const env = (await res.json().catch(() => null)) as
      | ApiEnvelope<unknown>
      | null;
    if (env && typeof env.code === "number" && env.code !== 0) {
      return new ApiError(
        env.code,
        env.message ?? "",
        env.data,
        res.status,
        env.request_id ?? undefined,
      );
    }
  }
  return new ApiError(
    1000,
    fallback.replace("{status}", String(res.status)),
    undefined,
    res.status,
    res.headers.get("x-request-id") ?? undefined,
  );
}

/** 取字节 + 元信息（批二百零五，GET 与 POST 共用一条路）。
 *
 *  必须在统一客户端里：csrf_mw 的成文判据是「非安全方法 + /api/v1 + 带了非空
 *  会话 cookie ⇒ X-CSRF-Token 与 cookie 逐字相等」，而缺 cookie 时去 GET /me/csrf
 *  续期那步（csrfHeaderAsync）是 api-client 私有的。组件内裸 fetch 两头都够不到
 *  ⇒ 活体 A/B：只带 flux_token 的 POST 回 403/2003「无权限」，补同名头回 200。
 *  浏览器同源必带 HttpOnly flux_token ⇒ 批量收种在真浏览器里整条不可用。
 *
 *  skipped：服务端 batch.rs 回 X-Batch-Skipped（少发几件），这个数必须交回调用方
 *  ——文件名与提示都按**实际件数**报，不许按请求数猜。 */
export async function blobRequest(
  path: string,
  init?: RequestInit,
): Promise<{ data: ArrayBuffer; skipped: number }> {
  const lang = acceptLanguage();
  const res = await fetch(`${baseUrl()}${path}`, {
    ...init,
    headers: {
      ...(lang ? { "Accept-Language": lang } : {}),
      ...(await csrfHeaderAsync(init?.method, null)),
      ...init?.headers,
    },
  });
  if (res.status === 401) {
    throw new ApiError(2001, netMsg(lang, "httpPleaseLogin"));
  }
  if (!res.ok) throw await toApiError(res, netMsg(lang, "httpDownloadFail"));
  const data = await res.arrayBuffer();
  return { data, skipped: Number(res.headers.get("x-batch-skipped")) || 0 };
}

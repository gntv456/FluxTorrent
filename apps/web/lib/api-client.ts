import type { ApiEnvelope, Page, PageParams } from "@fluxtorrent/domain-types";
import { LOCALE_COOKIE } from "@/i18n/config";

/**
 * 统一 API 客户端（方案 §8.3.2：组件内禁止裸 fetch）。
 * 双地址：服务端 RSC 用内网直连（API_SERVER_URL），浏览器用公开地址（NEXT_PUBLIC_API_URL）。
 */

/** 浏览器侧：登录/登出时同步会话标记 cookie（12h，与 JWT 24h 保守对齐）。
 *  安全收敛（P1）：token 已不再进 localStorage/JS 可读 cookie——登录接口经
 *  Set-Cookie 下发 HttpOnly flux_token（根路径 /），浏览器 fetch 自动携带；
 *  flux.session 仅为 middleware 存在性标记；服务端 RSC 需要转发 Bearer 时，
 *  由 Next rewrites 转发的请求同样自动带 flux_token cookie（api 侧
 *  require_auth 已接受 Cookie 凭证，见 token_from_request）。 */
export const SESSION_COOKIE = "flux.session";

/** 登录态判定（P1 收敛）：token 已 HttpOnly 化（flux_token，根路径 /），
 *  JS 不可读；浏览器侧以 flux.session 标记 cookie 判断。 */
export function hasSessionCookie(): boolean {
  if (typeof document === "undefined") return false;
  return document.cookie
    .split("; ")
    .some(
      (c) => c.startsWith("flux.session=") && c.length > "flux.session=".length,
    );
}

export function setSessionCookie(loggedIn: boolean): void {
  if (typeof document === "undefined") return;
  if (loggedIn) {
    document.cookie = `${SESSION_COOKIE}=1; path=/; max-age=43200; samesite=lax`;
  } else {
    document.cookie = `${SESSION_COOKIE}=; path=/; max-age=0; samesite=lax`;
  }
}

function baseUrl(): string {
  if (typeof window === "undefined") {
    // 服务端（RSC/容器内）：直连 api 服务
    return process.env.API_SERVER_URL ?? "http://localhost:8080";
  }
  // 浏览器：同源相对路径，经 Next rewrites 转发（免 CORS、免暴露 API 端口）
  return process.env.NEXT_PUBLIC_API_URL ?? "";
}

/** 浏览器侧：读语言 Cookie → Accept-Language，后端错误消息按语言返回 */
function acceptLanguage(): string | undefined {
  if (typeof window === "undefined") return undefined;
  const m = document.cookie.match(
    new RegExp(`(?:^|;\\s*)${LOCALE_COOKIE}=([^;]+)`),
  );
  return m ? decodeURIComponent(m[1]) : undefined;
}

/** 公开内部 helper：组件内需要裸 fetch（multipart 上传等）时复用同一 baseUrl/语言口径 */
export const rawFetchHelpers = {
  base: baseUrl,
  lang: acceptLanguage,
};

export class ApiError extends Error {
  constructor(
    public readonly code: number,
    message: string,
    /** 信封 data：站点设定的字段级校验错误等结构化的错误载荷 */
    public readonly data?: unknown,
  ) {
    super(message);
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  // 凭证（P1 收敛）：token 不再进 localStorage。浏览器依赖登录时 Set-Cookie 的
  // HttpOnly flux_token（同源 rewrites 转发，fetch 默认同源携带 cookie）；
  // 服务端 RSC 走 API_SERVER_URL 跨源直连，cookie 不随行——改为显式转发
  // 入站请求的 flux_token。
  let bearer: string | null = null;
  if (typeof window === "undefined") {
    const { cookies } = await import("next/headers");
    const store = await cookies();
    bearer = store.get("flux_token")?.value ?? null;
  }
  const lang = acceptLanguage();
  const res = await fetch(`${baseUrl()}${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(bearer ? { Authorization: `Bearer ${bearer}` } : {}),
      ...(lang ? { "Accept-Language": lang } : {}),
      ...init?.headers,
    },
    cache: "no-store",
  });
  // 非 JSON 响应（网关错误页等）归一化为 ApiError
  const ctype = res.headers.get("content-type") ?? "";
  if (!ctype.includes("application/json")) {
    throw new ApiError(1000, `服务异常（HTTP ${res.status}）`);
  }
  const body = (await res.json()) as ApiEnvelope<T>;
  if (body.code !== 0) {
    // 全局 401（审计修复 P1）：token 过期/被吊销时旧版只抛错，用户停留在
    // 「僵尸会话」页反复报错。清会话并带回跳地址跳登录页（登录页自身的
    // 401 与 auth/* 端点除外，避免登录前误跳）。
    if (
      body.code === 2001 &&
      typeof window !== "undefined" &&
      !path.startsWith("/api/v1/auth/")
    ) {
      setSessionCookie(false);
      const next = encodeURIComponent(
        window.location.pathname + window.location.search,
      );
      window.location.assign(`/login?next=${next}&expired=1`);
    }
    throw new ApiError(body.code, body.message, body.data);
  }
  return body.data;
}

/** 二进制下载（凭证由 HttpOnly cookie 自动携带） */
async function requestBlob(path: string): Promise<ArrayBuffer> {
  const lang = acceptLanguage();
  const res = await fetch(`${baseUrl()}${path}`, {
    headers: {
      ...(lang ? { "Accept-Language": lang } : {}),
    },
  });
  if (res.status === 401) {
    throw new ApiError(2001, "请先登录");
  }
  if (!res.ok) {
    throw new ApiError(1000, `下载失败（HTTP ${res.status}）`);
  }
  return res.arrayBuffer();
}

export const api = {
  get: <T>(path: string) => request<T>(path),
  getBlob: (path: string) => requestBlob(path),
  post: <T>(path: string, data?: unknown) =>
    request<T>(path, { method: "POST", body: JSON.stringify(data ?? {}) }),
  put: <T>(path: string, data?: unknown) =>
    request<T>(path, { method: "PUT", body: JSON.stringify(data ?? {}) }),
  del: <T>(path: string) => request<T>(path, { method: "DELETE" }),
  /** "PUT /api/v1/xxx {json}" 快捷调用（content-manage 内部用） */
  call: <T>(spec: string): Promise<T> => {
    const m = spec.match(/^(GET|POST|PUT|DELETE) (\S+)(?: (\{.*\}))?$/);
    if (!m) throw new ApiError(1002, "invalid call spec");
    const [, method, path, body] = m;
    return request<T>(path, {
      method,
      ...(body ? { body } : {}),
    });
  },
};

/** 游标分页列表 */
export async function paged<T>(
  resource: string,
  params: PageParams & Record<string, string | number | boolean | undefined>,
): Promise<Page<T>> {
  const qs = new URLSearchParams();
  if (params.cursor) qs.set("cursor", params.cursor);
  if (params.limit) qs.set("limit", String(params.limit));
  for (const [k, v] of Object.entries(params)) {
    if (["cursor", "limit"].includes(k) || v === undefined) continue;
    qs.set(k, String(v));
  }
  return request<Page<T>>(`${resource}?${qs.toString()}`);
}

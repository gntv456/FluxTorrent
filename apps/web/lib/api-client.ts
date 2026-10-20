import type { ApiEnvelope, Page, PageParams } from "@fluxtorrent/domain-types";
import { LOCALE_COOKIE } from "@/i18n/config";
import { apiServerBase } from "@/lib/api-upstream";
import { blobRequest, toApiError } from "@/lib/blob-error";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 统一 API 客户端（方案 §8.3.2：组件内禁止裸 fetch）。
 * 双地址：服务端 RSC 用内网直连（API_SERVER_URL），浏览器用公开地址（NEXT_PUBLIC_API_URL）。
 */

/** 网络层兜底文案（i18n 收口批）：api-client 不在 React 树内拿不到
 *  LocaleProvider 字典，这里按 locale cookie 取对应字典的网络段；
 *  取不到（SSR 无 cookie 等）回落 zh-CN 源字典。en/zh-TW 字典整本
 *  数千键，顶层静态 import 会把它拖进每个页面的 bundle——按需
 *  require（webpack 分包）只在此分支命中时加载。 */
const NET_DICTS: Record<string, () => { errors?: Record<string, string> }> = {
  en: () => require("@/i18n/en").en as { errors?: Record<string, string> },
  "zh-TW": () =>
    require("@/i18n/zh-TW").zhTW as { errors?: Record<string, string> },
};
export function netMsg(
  locale: string | undefined,
  key: "httpServerError" | "httpDownloadFail" | "httpPleaseLogin",
): string {
  const load = locale ? NET_DICTS[locale] : undefined;
  if (load) {
    const v = load().errors?.[key];
    if (v) return v;
  }
  return (zhCN.errors as unknown as Record<string, string>)?.[key] ?? "";
}

/** 浏览器侧：登录/登出时同步会话标记 cookie（12h，与 JWT 24h 保守对齐）。
 *  安全收敛（P1）：token 已不再进 localStorage/JS 可读 cookie——登录接口经
 *  Set-Cookie 下发 HttpOnly flux_token（根路径 /），浏览器 fetch 自动携带；
 *  flux.session 仅为 middleware 存在性标记；服务端 RSC 需要转发 Bearer 时，
 *  由 Next rewrites 转发的请求同样自动带 flux_token cookie（api 侧
 *  require_auth 已接受 Cookie 凭证，见 token_from_request）。 */
export const SESSION_COOKIE = "flux.session";

/** CSRF 双提交 cookie 名（后端 csrf.rs 的契约，后端改这里要同步）。 */
export const CSRF_COOKIE = "flux_csrf";
/** CSRF 请求头名（与后端 CSRF_HEADER 同源）。 */
export const CSRF_HEADER = "x-csrf-token";

/** 浏览器侧：读 flux_csrf cookie（双提交模式要求它**非** HttpOnly——
 *  前端要能读出来放进请求头，服务端比对 cookie 与 header 是否逐字相等）。 */
export function csrfToken(): string | null {
  if (typeof document === "undefined") return null;
  const m = document.cookie.match(
    new RegExp("(?:^|;\\s*)" + CSRF_COOKIE + "=([^;]+)"),
  );
  return m ? decodeURIComponent(m[1]) : null;
}

/** 浏览器侧 CSRF 按需续期（缺 cookie 时去后端要一份）。
 *
 *  后端 `csrf_mw` 的判据是「非安全方法 + /api/v1 + 带了非空会话 cookie」就必须
 *  cookie 与 `X-CSRF-Token` 逐字相等；而全站唯一下发 `flux_csrf` 的口是
 *  `GET /api/v1/me/csrf`（`csrf::set_cookie` 在后端零调用点，登录响应也不带），
 *  前端此前从不打这个口 ⇒ 浏览器里所有写操作回 403「无权限」（2026-10-16 实测：
 *  真表单登录后 cookie 只有 flux.locale/flux_token/flux.session）。这里在首次写之前
 *  续期一次，界面写口才可用；续期失败仍照原样发出，让后端那句出声。 */
export async function csrfHeaderAsync(
  method: string | undefined,
  bearer: string | null,
): Promise<Record<string, string>> {
  const m = (method ?? "").toUpperCase();
  if (m === "" || m === "GET" || m === "HEAD" || m === "OPTIONS") return {};
  const t = csrfToken();
  if (t) return { [CSRF_HEADER]: t };
  // 服务端 RSC 走 Bearer 直连、不带会话 cookie，闸本身不会触发
  if (typeof window === "undefined") return {};
  try {
    await fetch(`${baseUrl()}/api/v1/me/csrf`, {
      credentials: "same-origin",
      cache: "no-store",
      headers: bearer ? { Authorization: `Bearer ${bearer}` } : {},
    });
  } catch {
    /* 不吞出声：拿不到 cookie 就照原样发，由后端回 403 给界面显示 */
  }
  const fresh = csrfToken();
  return fresh ? { [CSRF_HEADER]: fresh } : {};
}

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
    document.cookie =
      `${SESSION_COOKIE}=1; path=/; max-age=43200; samesite=lax`;
  } else {
    document.cookie = `${SESSION_COOKIE}=; path=/; max-age=0; samesite=lax`;
  }
}

export function baseUrl(): string {
  if (typeof window === "undefined") {
    // 服务端（RSC/容器内）：直连 api 服务（单源见 api-upstream.ts）
    return apiServerBase();
  }
  // 浏览器：同源相对路径，经 Next rewrites 转发（免 CORS、免暴露 API 端口）
  return process.env.NEXT_PUBLIC_API_URL ?? "";
}

/** 浏览器侧：读语言 Cookie → Accept-Language，后端错误消息按语言返回 */
export function acceptLanguage(): string | undefined {
  if (typeof window === "undefined") return undefined;
  const m = document.cookie.match(
    new RegExp(`(?:^|;\\s*)${LOCALE_COOKIE}=([^;]+)`),
  );
  return m ? decodeURIComponent(m[1]) : undefined;
}

export const rawFetchHelpers = {
  base: baseUrl,
  lang: acceptLanguage,
  csrf: (method?: string) => csrfHeaderAsync(method, null),
};

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
  // revalidate（P1-2 布局缓存）：仅服务端 RSC 且调用方显式传值时用
  // Next data cache（next.revalidate 与 cache:'no-store' 互斥，二选一）。
  // 默认仍 no-store——页面自身数据绝不过期。浏览器侧忽略 revalidate。
  const serverCache =
    typeof window === "undefined" &&
    (init as { next?: { revalidate?: number } } | undefined)?.next?.revalidate;
  const res = await fetch(`${baseUrl()}${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(bearer ? { Authorization: `Bearer ${bearer}` } : {}),
      ...(lang ? { "Accept-Language": lang } : {}),
      // CSRF 双提交：写方法回填 token（服务端 csrf_mw 比对 cookie/header）
      ...(await csrfHeaderAsync(init?.method, bearer)),
      ...init?.headers,
    },
    ...(serverCache ? {} : { cache: "no-store" as const }),
  });
  // 非 JSON 响应（网关错误页等）归一化为 ApiError；带响应头 request_id
  const ctype = res.headers.get("content-type") ?? "";
  if (!ctype.includes("application/json")) {
    throw await toApiError(res, netMsg(lang, "httpServerError"));
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
    throw new ApiError(
      body.code,
      body.message,
      body.data,
      res.status,
      body.request_id,
    );
  }
  return body.data;
}

/** 二进制下载（凭证由 HttpOnly cookie 自动携带）。实现搬在 blob-error.ts 的
 *  blobRequest——本文件是基线 301 行的载体只许瘦不许胖，且 GET/POST 要共用
 *  同一条 CSRF + 失败归一尺。 */
async function requestBlob(path: string): Promise<ArrayBuffer> {
  return (await blobRequest(path)).data;
}

export const api = {
  /** revalidate（可选，P1-2）：服务端 RSC 的跨请求缓存秒数（Next data
   *  cache）；浏览器侧忽略。仅布局级「每页都取、允许短陈旧」的数据
   *  （site-profile / menu-items）传值，页面自身数据保持 no-store。 */
  get: <T>(path: string, revalidate?: number) =>
    request<T>(
      path,
      revalidate !== undefined && typeof window === "undefined"
        ? { next: { revalidate } }
        : undefined,
    ),
  getBlob: (path: string) => requestBlob(path),
  /** POST 取字节（批量收种）：CSRF 续期与失败归一与 GET 同一条路 */
  postBlob: (path: string, body?: unknown) =>
    blobRequest(path, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body ?? {}),
    }),
  post: <T>(path: string, data?: unknown) =>
    request<T>(path, { method: "POST", body: JSON.stringify(data ?? {}) }),
  put: <T>(path: string, data?: unknown) =>
    request<T>(path, { method: "PUT", body: JSON.stringify(data ?? {}) }),
  patch: <T>(path: string, data?: unknown) =>
    request<T>(path, { method: "PATCH", body: JSON.stringify(data ?? {}) }),
  del: <T>(path: string, data?: unknown) =>
    request<T>(path, { method: "DELETE", body: JSON.stringify(data) }),
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
export async function paged<T, P extends Page<T> = Page<T>>(
  resource: string,
  params: PageParams & Record<string, string | number | boolean | undefined>,
): Promise<P> {
  const qs = new URLSearchParams();
  if (params.cursor) qs.set("cursor", params.cursor);
  if (params.limit) qs.set("limit", String(params.limit));
  for (const [k, v] of Object.entries(params)) {
    if (["cursor", "limit"].includes(k) || v === undefined) continue;
    qs.set(k, String(v));
  }
  return request<P>(`${resource}?${qs.toString()}`);
}

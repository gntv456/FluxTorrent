import type {
  ApiEnvelope,
  Page,
  PageParams,
} from "@fluxtorrent/domain-types";
import { LOCALE_COOKIE } from "@/i18n/config";

/**
 * 统一 API 客户端（方案 §8.3.2：组件内禁止裸 fetch）。
 * 双地址：服务端 RSC 用内网直连（API_SERVER_URL），浏览器用公开地址（NEXT_PUBLIC_API_URL）。
 */

/** 浏览器侧：登录/登出时同步会话 cookie（12h，与 JWT 24h 保守对齐）。
 *  flux.session 是 middleware 存在性标记；flux.token 供 RSC 服务端读取转发 Bearer。 */
export const SESSION_COOKIE = "flux.session";
export const TOKEN_COOKIE = "flux.token";

export function setSessionCookie(token: string | null): void {
  if (typeof document === "undefined") return;
  if (token) {
    document.cookie = `${SESSION_COOKIE}=1; path=/; max-age=43200; samesite=lax`;
    document.cookie = `${TOKEN_COOKIE}=${token}; path=/; max-age=43200; samesite=lax`;
  } else {
    document.cookie = `${SESSION_COOKIE}=; path=/; max-age=0; samesite=lax`;
    document.cookie = `${TOKEN_COOKIE}=; path=/; max-age=0; samesite=lax`;
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

export class ApiError extends Error {
  constructor(
    public readonly code: number,
    message: string,
  ) {
    super(message);
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  // 浏览器：localStorage；服务端 RSC：请求 cookie 里的 flux.token（登录时同步写入）
  let token: string | null = null;
  if (typeof window !== "undefined") {
    token = localStorage.getItem("flux.token");
  } else {
    const { cookies } = await import("next/headers");
    const store = await cookies();
    token = store.get("flux.token")?.value ?? null;
  }
  const lang = acceptLanguage();
  const res = await fetch(`${baseUrl()}${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
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
    throw new ApiError(body.code, body.message);
  }
  return body.data;
}

/** 二进制下载（携带鉴权） */
async function requestBlob(path: string): Promise<ArrayBuffer> {
  const token = localStorage.getItem("flux.token");
  const lang = acceptLanguage();
  const res = await fetch(`${baseUrl()}${path}`, {
    headers: {
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
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

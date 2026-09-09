import type {
  ApiEnvelope,
  Page,
  PageParams,
} from "@fluxtorrent/domain-types";

/**
 * 统一 API 客户端（方案 §8.3.2：组件内禁止裸 fetch）。
 * 双地址：服务端 RSC 用内网直连（API_SERVER_URL），浏览器用公开地址（NEXT_PUBLIC_API_URL）。
 */

function baseUrl(): string {
  if (typeof window === "undefined") {
    // 服务端（RSC/容器内）：直连 api 服务
    return process.env.API_SERVER_URL ?? process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";
  }
  return process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";
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
  const token =
    typeof window !== "undefined" ? localStorage.getItem("flux.token") : null;
  const res = await fetch(`${baseUrl()}${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
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
  const res = await fetch(`${baseUrl()}${path}`, {
    headers: token ? { Authorization: `Bearer ${token}` } : {},
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

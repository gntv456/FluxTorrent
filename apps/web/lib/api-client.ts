import type {
  ApiEnvelope,
  Page,
  PageParams,
} from "@fluxtorrent/domain-types";

/**
 * 统一 API 客户端（方案 §8.3.2：组件内禁止裸 fetch）。
 * 职责：信封解包、错误码映射、Bearer 注入。
 */

const BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

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
  const res = await fetch(`${BASE}${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...init?.headers,
    },
    cache: "no-store",
  });
  const body = (await res.json()) as ApiEnvelope<T>;
  if (body.code !== 0) {
    throw new ApiError(body.code, body.message);
  }
  return body.data;
}

export const api = {
  get: <T>(path: string) => request<T>(path),
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

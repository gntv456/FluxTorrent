import { rawFetchHelpers } from "@/lib/api-client";

/** 发布提交（POST /api/v1/torrents）的 HTTP 层。
 *
 *  从 upload-form.tsx 按域拆出（300 行门禁）。这一层只负责「把表单发出去、
 *  把失败归一成可读提示」，业务码（1002 字段级信封等）仍由调用方判。
 *
 *  两处历史坑（2026-10-16 修）：
 *  1. **CSRF 双提交**：后端 csrf_mw 对「非安全方法 + /api/v1 + 带会话
 *     cookie」要求 flux_csrf cookie 与 X-CSRF-Token 逐字相等。此前这里是
 *     裸 fetch、不带该头 ⇒ 浏览器里提交发布恒回 403「无权限」，
 *     连站长账号也发不出种子（api.post 系列已由 218bbd7a 修过，
 *     裸 fetch 这条路一直漏着）。
 *  2. **先判状态再解析**：403/401 常带非 JSON 响应体（或空体），旧实现
 *     直接 res.json() 会抛异常、落进 catch 显示「网络错误」——把权限问题
 *     说成网络问题，发布失败却看不出真实原因。
 */

export interface TorrentPostResp {
  code: number;
  data?: {
    id?: number;
    group_suggest?: {
      same_source?: boolean;
      torrent_id?: number;
      name?: string;
    };
  };
}

export type PostTorrentResult =
  | { ok: true; body: TorrentPostResp }
  /** 已失败：msg 是给用户看的归一化文案，调用方直接 return */
  | { ok: false; msg: string };

export async function postTorrent(
  form: FormData,
  msgs: {
    /** 403/401 的提示（调用方传 dict.upload.noPermission） */
    noPermission: string;
    /** 其它状态码的提示工厂（传 (s) => fmt(dict.upload.submitFailStatus, …)） */
    failStatus: (status: number) => string;
  },
): Promise<PostTorrentResult> {
  const csrf = await rawFetchHelpers.csrf("POST");
  const lang = rawFetchHelpers.lang();
  const res = await fetch(`${rawFetchHelpers.base()}/api/v1/torrents`, {
    method: "POST",
    headers: { ...csrf, ...(lang ? { "Accept-Language": lang } : {}) },
    body: form,
  });
  const fail =
    res.status === 403 || res.status === 401
      ? msgs.noPermission
      : msgs.failStatus(res.status);
  if (!res.ok) return { ok: false, msg: fail };
  try {
    return { ok: true, body: (await res.json()) as TorrentPostResp };
  } catch {
    return { ok: false, msg: fail };
  }
}

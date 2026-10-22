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
  if (res.status === 403 || res.status === 401) {
    return { ok: false, msg: msgs.noPermission };
  }
  // P-569：1002/3003/3004 都以 HTTP 400 返回，业务原因只在信封
  // message/data 里。旧实现非 2xx 直接丢体 ⇒ 界面永远只有「服务端
  // 返回 400」，P-303 的「拒单原因必须露出」在真实状态码下走不到。
  if (!res.ok) {
    const body = res.status < 500 ? await readEnvelope(res) : null;
    if (body && body.code !== 0) return { ok: true, body };
    return { ok: false, msg: msgs.failStatus(res.status) };
  }
  const body = await readEnvelope(res);
  if (!body) return { ok: false, msg: msgs.failStatus(res.status) };
  return { ok: true, body };
}

/** 解析信封；非 JSON（CSRF/反代错误页）或缺数字 code 时返回 null。 */
async function readEnvelope(res: Response): Promise<TorrentPostResp | null> {
  try {
    const body = (await res.json()) as TorrentPostResp;
    if (body && typeof body === "object" && typeof body.code === "number") {
      return body;
    }
  } catch {
    // 非 JSON：交给调用方的通用失败文案
  }
  return null;
}

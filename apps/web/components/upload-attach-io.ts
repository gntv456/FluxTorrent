/** 图床上传 IO（0100，从 upload-form-files.tsx 拆出：300 行门禁 +
 *  i18n 收口批）。POST /attachments，成功后把 [img]URL[/img]（图片）
 *  或 [url=...][file][/url]（其他附件）追加进简介。只做 IO 与文案
 *  组装，busy/msg 状态由调用方管理。 */

import type { Dict } from "@/i18n/zh-CN";

type DescrSetter = (v: string | ((prev: string) => string)) => void;

export async function attachUpload(
  file: File,
  dict: Dict,
  setDescr: DescrSetter,
): Promise<{ msg: string }> {
  const fd = new FormData();
  fd.append("file", file);
  // 凭证由 HttpOnly flux_token cookie 自动携带（P1 收敛，token 不进 JS）
  const res = await fetch("/api/v1/attachments", { method: "POST", body: fd });
  const j = (await res.json()) as {
    data?: { url?: string; deduplicated?: boolean };
    message?: string;
  };
  if (!res.ok || !j.data?.url) {
    throw new Error(j.message ?? dict.upload.attachFail);
  }
  const tag = file.type.startsWith("image/")
    ? `[img]${j.data.url}[/img]`
    : `[url=${j.data.url}]${file.name}[/url]`;
  setDescr((prev) => (prev.trim() ? `${prev.trim()}\n\n` : "") + tag);
  return {
    msg: j.data.deduplicated
      ? dict.upload.attachDedup
      : dict.upload.attachOk,
  };
}

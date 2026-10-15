/** 字幕附件上传（从 subtitle-request-panel.tsx 挪出，300 行门禁腾位）：
 *  multipart 到 attachments 拿 sha256（上传表单/求字幕/种子面板共用）。 */

import { ApiError, rawFetchHelpers } from "@/lib/api-client";

export async function uploadAttachment(file: File): Promise<string> {
  const form = new FormData();
  form.append("file", file);
  const lang = rawFetchHelpers.lang();
  const upRes = await fetch(rawFetchHelpers.base() + "/api/v1/attachments", {
    method: "POST",
    headers: {
      ...(lang ? { "Accept-Language": lang } : {}),
    },
    body: form,
  });
  const upBody = (await upRes.json()) as {
    code: number;
    message?: string;
    data?: { sha256: string };
  };
  if (upBody.code !== 0 || !upBody.data?.sha256) {
    throw new ApiError(upBody.code, upBody.message ?? "upload failed");
  }
  return upBody.data.sha256;
}

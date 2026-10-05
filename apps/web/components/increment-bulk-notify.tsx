"use client";

import type { Dict } from "@/i18n/zh-CN";

/** 通知行（0286 从 increment-bulk.tsx 拆出守 300 行门禁）：PM 发送者 + 邮件双通道。 */

export function BulkNotifyRow(props: {
  t: Dict["adminBulk"];
  email: boolean;
  setEmail: (v: boolean) => void;
}) {
  const { t, email, setEmail } = props;
  return (
    <>
      <tr>
        <td className="rowhead">{t.notifyWay}</td>
        <td className="rowfollow">
          <label className="inline-flex items-center gap-1.5 text-sm">
            <input
              type="checkbox"
              checked={email}
              onChange={(e) => setEmail(e.target.checked)}
            />
            {t.emailToo}
          </label>
        </td>
      </tr>
    </>
  );
}

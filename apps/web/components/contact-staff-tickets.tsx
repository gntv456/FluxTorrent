"use client";

import { useI18n } from "@/i18n/client";

/** PM 管理组·我的工单进度（从 components/contact-staff.tsx 按域拆出）：
 *  提交后可见进度（0新 1处理中 2已答复待确认 3关闭），
 *  已答复可「确认关闭」闭环（审计修复 P1）。 */

export interface MyTicket {
  id: number;
  subject: string;
  ticket_status: number; // 0新 1处理中 2已答复待确认 3关闭
  answer: string | null;
  answered_at: string | null;
  created_at: string;
}

// BBCode 颜色/字体下拉选项（参考站 contactstaff.php 原版列表）
export const COLORS = [
  "Black",
  "Sienna",
  "Dark Olive Green",
  "Dark Green",
  "Navy",
  "Indigo",
  "Dark Slate Gray",
  "Dark Red",
  "Dark Orange",
  "Olive",
  "Green",
  "Teal",
  "Blue",
  "Slate Gray",
  "Dim Gray",
  "Red",
  "Sandy Brown",
  "Yellow Green",
  "Sea Green",
  "Royal Blue",
  "Purple",
  "Gray",
  "Magenta",
  "Orange",
  "Yellow",
  "Lime",
  "Cyan",
  "Deep Sky Blue",
  "Pink",
  "Wheat",
  "Lemon Chiffon",
  "Pale Green",
  "Light Blue",
  "Plum",
  "White",
];

export const FONTS = [
  "Arial",
  "Arial Black",
  "Book Antiqua",
  "Century Gothic",
  "Comic Sans MS",
  "Courier New",
  "Garamond",
  "Georgia",
  "Impact",
  "Lucida Console",
  "Microsoft Sans Serif",
  "Palatino Linotype",
  "System",
  "Tahoma",
  "Times New Roman",
  "Trebuchet MS",
  "Verdana",
];

export function MyTickets({
  tickets,
  onConfirm,
}: {
  tickets: MyTicket[] | null;
  onConfirm: (id: number) => void;
}) {
  const { dict, locale } = useI18n();
  const t = dict.contactstaff;

  return (
    <section className="contactstaff-wrap" aria-label={t.myTickets}>
      <h2>{t.myTickets}</h2>
      {tickets === null && (
        <p className="contactstaff-hint">{t.loading ?? "…"}</p>
      )}
      {tickets?.length === 0 && (
        <p className="contactstaff-hint">{t.noTickets}</p>
      )}
      {tickets && tickets.length > 0 && (
        <table className="nexus-table">
          <tbody>
            {tickets.map((tk) => (
              <tr key={tk.id}>
                <td className="rowhead">{tk.subject}</td>
                <td className="rowfollow">
                  <span className="sticker">
                    {[t.stNew, t.stProcessing, t.stAnswered, t.stClosed][
                      tk.ticket_status
                    ] ?? tk.ticket_status}
                  </span>
                  <p className="contactstaff-hint">
                    {new Date(tk.created_at).toLocaleString(
                      locale === "zh-CN"
                        ? "zh-CN"
                        : locale === "zh-TW"
                          ? "zh-TW"
                          : "en-US",
                    )}
                  </p>
                  {tk.answer && (
                    <blockquote className="contactstaff-preview">
                      <b>{t.replyLabel}</b>
                      <p>{tk.answer}</p>
                    </blockquote>
                  )}
                  {tk.ticket_status === 2 && (
                    <input
                      type="button"
                      className="btn"
                      value={t.confirmClose}
                      onClick={() => onConfirm(tk.id)}
                    />
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

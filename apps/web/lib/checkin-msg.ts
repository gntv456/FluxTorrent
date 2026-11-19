/** 0501 签到成功文案拼装：券反馈断链收口（三调用点共用）。
 *  后端一直下发 gacha_ticket / gacha_ticket_milestone，此前前端零展示。 */
export function withTicketNote(
  base: string,
  t: { checkinTicket: string; checkinTicketMilestone: string },
  r: { gacha_ticket?: boolean; gacha_ticket_milestone?: boolean },
): string {
  if (r.gacha_ticket_milestone) return base + t.checkinTicketMilestone;
  if (r.gacha_ticket) return base + t.checkinTicket;
  return base;
}

/** POST /attendance/checkin 响应面（券反馈字段 0501 起消费）。 */
export interface CheckinResp {
  reward: number;
  streak: number;
  gacha_ticket?: boolean;
  gacha_ticket_milestone?: boolean;
}

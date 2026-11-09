import type { HomeData } from "@/components/home-data";

/** 签到日历板块共享类型与工具（AttendanceCard / Strip / Sheet 共用）。 */

export interface AttendanceShape {
  month: string;
  streak: number;
  total_days: number;
  checked_today: boolean;
  calendar: { date: string; day: number; done: boolean; reward: number }[];
  makeup_cards?: number;
}

export function attOf(home: HomeData): AttendanceShape | null {
  const a = home.attendance as AttendanceShape & { off?: boolean };
  if (!a || a.off || !Array.isArray(a.calendar)) return null;
  return a;
}

/** 站点今天（UTC+8 日界，与后端生成日历同口径）。 */
export function todayStr(): string {
  return new Date(Date.now() + 8 * 3600 * 1000).toISOString().slice(0, 10);
}

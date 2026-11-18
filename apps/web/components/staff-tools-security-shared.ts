/**
 * 安全域面板·共享类型（从 components/staff-tools-security.tsx 按域拆出）：
 * 封禁 / 批量邮件 / 邮箱规则 / IP 测试结果行类型。
 */

export interface BanItem {
  id: number;
  ip: string;
  reason: string | null;
  banned_by: string | null;
  created_at: string;
}

export interface MailItem {
  id: number;
  subject: string;
  recipients: number;
  created_at: string;
  sender: string | null;
  /** 0499 圈选人群（dev.22 前的旧行无此列） */
  audience?: string | null;
}

export interface EmailBan {
  id: number;
  pattern: string;
  mode: string;
  note: string | null;
  created_by: string | null;
  created_at: string;
}

export interface TestIpResult {
  ip: string;
  banned: boolean;
  reason: string | null;
  by: string | null;
  seen_users: string[];
}

/** 一次性邮箱域名库行（0370 M4 第 ④ 道闸） */
export interface EmailDomainRow {
  domain: string;
  source: string;
  created_at: string;
}

/** audience 存库值 → 展示文案（旧行 null 显示全员/—）。 */
export function audLabel(
  v: string | null | undefined,
  t: {
    audAll: string;
    audActive30: string;
    audDormant90: string;
    audVip: string;
    audClass: string;
  },
): string {
  switch (v) {
    case undefined:
    case "":
    case "all":
      return t.audAll;
    case null:
      return "—";
    case "active30":
      return t.audActive30;
    case "dormant90":
      return t.audDormant90;
    case "vip":
      return t.audVip;
    default:
      return v.startsWith("class:")
        ? t.audClass.replace("{n}", v.slice(6))
        : v;
  }
}

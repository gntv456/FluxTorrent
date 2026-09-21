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

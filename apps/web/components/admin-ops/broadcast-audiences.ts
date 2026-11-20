/** 站内信广播受众词表（从 admin-ops-broadcast.tsx 拆出，300 行门禁）。
 *  key 是后端 massmail audience 圈选协议的取值（dev.22 批口径）。 */
export const AUDIENCES = [
  ["all", "全员"],
  ["active30", "30 天活跃"],
  ["dormant90", "90 天未活跃"],
  ["vip", "VIP/捐赠"],
  ["class:1", "等级≥1"],
  ["class:5", "等级≥5"],
  ["class:98", "等级≥98"],
] as const;

export const aAudienceLabel = (key: string) =>
  AUDIENCES.find(([k]) => k === key)?.[1] ?? key;

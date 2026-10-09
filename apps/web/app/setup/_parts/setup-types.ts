/** 安装向导共享类型（从 setup/page.tsx 按域拆出，300 行门禁）。 */

export interface Pack {
  code: string;
  name: string;
  description: string;
}

export interface Status {
  done: boolean;
  has_admin: boolean;
  /** 当前站名（G12）：Step2「留空」的后果要看得到 */
  site_name?: string;
  /** 引导 root 仍是临时密码（G12 复验发现）：Step2 就地改密引导的开关 */
  root_temp_password?: boolean;
  /** 该临时密码管理员的账号名（后端回传，别让站长凭空猜） */
  temp_admin_username?: string;
  /** 其口令是否仍为引导默认值 password123（哈希比对得出，非按用户名假定） */
  temp_admin_default_pw?: boolean;
  packs: Pack[];
  /** 开站 checklist（0209 P2-12） */
  checklist?: {
    announce_local: boolean;
    smtp_unset: boolean;
    registration_mode: string;
  };
}

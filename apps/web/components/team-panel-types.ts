/** 保种协作类型（从 team-panel.tsx 拆出，300 行门禁）。 */

export type TeamRow = {
  team_id: number;
  torrent_id: number;
  torrent_name: string;
  leader_name: string;
  member_count: number;
  team_size_max: number;
  deadline_at: string | null;
  seeders: number;
  joined: boolean;
};

export type Member = {
  uid: number;
  username: string;
  is_leader: boolean;
  join_status: number;
  delta_seconds: number;
};

export type MineRow = {
  team_id: number;
  status: number;
  team_size_max: number;
  torrent_id: number | null;
  torrent_name: string | null;
  members: Member[];
  /// 当前用户是否是队长（决定是否给出「退出」按钮）
  viewer_is_leader: boolean;
};

export type ListResp = { enabled: boolean; list: TeamRow[] };
export type MineResp = { enabled: boolean; list: MineRow[] };

/**
 * 用户公开主页共享类型（从 app/(main)/users/[id]/page.tsx 按域拆出）：
 * 资料全景 ProfileData / 种子列表 TorrentLists / 标签页 TABS。
 * 仅类型与常量，无运行时代码。
 */

export interface Profile {
  id: number;
  username: string;
  title: string | null;
  avatar_url: string | null;
  class_id: number;
  class_name: string | null;
  uploaded: number;
  downloaded: number;
  donor: boolean;
  created_at: string;
  last_seen_at: string | null;
  seeding: number;
  leeching: number;
  uploads: number;
  comments: number;
  medals: number;
}

export interface RecentUpload {
  id: number;
  name: string;
  small_descr: string | null;
  size: number;
  created_at: string;
}

export interface RecentComment {
  torrent_id: number;
  body: string;
  created_at: string;
}

export interface TorrentHistRow {
  torrent_id: number;
  name: string;
  size: number;
  seeders: number;
  leechers: number;
  seeding: boolean;
}

export interface ProfileData {
  profile: Profile;
  gender: string | null;
  country: string | null;
  isp: string | null;
  upload_speed: number | null;
  download_speed: number | null;
  info: string | null;
  signature: string | null;
  online: boolean;
  real_uploaded: number;
  real_downloaded: number;
  seed_seconds: number;
  hr_unresolved: number;
  hr_limit: number;
  seeding_size: number;
  spark_balance: number;
  month_seed_earn: number;
  completed_snatches: number;
  invites_pending: number;
  inviter_name: string | null;
  inviter_id: number | null;
  client_agent: string | null;
  worn_medals: {
    id: number;
    name: string;
    description: string | null;
    asset_ref: string | null;
  }[];
  avatar_frame_css: string | null;
  avatar_frame_image: string | null;
  achievements: number;
  next_class: {
    class_id: number;
    name: string;
    uploaded: number;
    uploaded_need: number;
    download_count: number;
    download_count_need: number;
    seed_hours: number;
    seed_hours_need: number;
    account_age_days: number;
    account_age_days_need: number;
  } | null;
  recent_posts: [number, number, string, string][];
  recent_uploads: RecentUpload[];
  recent_comments: RecentComment[];
  /** 字幕作品摘要（0146 P2-2：公开归属的过审字幕；匿名上传不计入） */
  subtitle_count: number;
  subtitle_downloads: number;
  /** 字幕身份（0149：certified / gold；null = 无） */
  subtitle_cert: "certified" | "gold" | null;
}

export type TorrentLists = {
  uploads: TorrentHistRow[];
  seeding: TorrentHistRow[];
  leeching: TorrentHistRow[];
  completed: TorrentHistRow[];
  incomplete: TorrentHistRow[];
  preserved: TorrentHistRow[];
};

/** 憨憨式标签页：个人中心（默认）+ 发布种子/当前做种/当前下载/完成/未完成/保种区 + 论坛动态/评论 */
export const TABS = [
  "center",
  "uploads",
  "seeding",
  "leeching",
  "completed",
  "incomplete",
  "preserved",
  "posts",
  "comments",
] as const;
export type Tab = (typeof TABS)[number];

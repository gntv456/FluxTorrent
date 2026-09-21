/** 字幕区共享契约（从 components/subtitle-board.tsx 按域拆出；0146 扩展） */

export interface SubtitleRow {
  id: number;
  torrent_id: number | null;
  username: string | null;
  title: string;
  lang: string | null;
  lang_id?: number | null;
  downloads: number;
  size: number | null;
  ext?: string | null;
  rating?: number | null;
  rating_count?: number;
  created_at: string;
  /** 上传者 id（anon 行也有；前端判定编辑/删除入口） */
  user_id?: number;
  verified?: boolean;
}

/** 列表响应（0146 分页信封；旧数组形态兼容到 items） */
export interface SubtitleListResp {
  items: SubtitleRow[];
  total: number;
  page: number;
  per_page: number;
}

/** 语言字典行（GET /api/v1/subtitles/langs） */
export interface SubtitleLang {
  id: number;
  code: string;
  name: string;
  flag: string | null;
  position: number;
}

/** 求字幕悬赏行（GET /api/v1/subtitles/requests） */
export interface SubtitleRequestRow {
  id: number;
  username: string | null;
  torrent_id: number | null;
  lang: string;
  descr: string | null;
  bounty: number;
  contributors: { user_id?: number; amount?: number }[];
  status: number;
  fulfilled_subtitle_id: number | null;
  created_at: string;
}

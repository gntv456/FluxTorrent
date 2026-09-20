/** 字幕区共享契约（从 components/subtitle-board.tsx 按域拆出） */

export interface SubtitleRow {
  id: number;
  torrent_id: number | null;
  username: string | null;
  title: string;
  lang: string | null;
  downloads: number;
  size: number | null;
  created_at: string;
}

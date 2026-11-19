// 头栏全局搜索的联想结果契约（0283 P1-6 → W-25）。
// 从 global-search.tsx 外移，让主组件守住 300 行上限（line_limit_guard）。
export interface TorrentHit {
  id: number;
  name: string;
  seeders: number;
}
export interface TopicHit {
  topic_id: number;
  title: string;
}
export interface Hits {
  torrents: TorrentHit[];
  topics: TopicHit[];
}

export const EMPTY: Hits = { torrents: [], topics: [] };

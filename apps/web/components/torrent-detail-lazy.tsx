"use client";

import dynamic from "next/dynamic";
import { SkeletonRows } from "@/components/ui/skeleton";

/** 种子详情页重内容懒壳（2026-11-11 提速批）：
 *  三件都是「普通访客多数用不到、挂载即有成本」的重内容——
 *  - TorrentManage：staff/发布者编辑器（348 行 + UploadDescrBlock 拖进的
 *    form-field 链，page.tsx 已外部门卫，普通访客连挂载都不挂）
 *  - TorrentPeers：挂载即拉 tracker swarm 快照 API；收进 Fold 门卫后
 *    展开瞬间才挂载 → 请求也推迟到展开
 *  - FileTree：文件树渲染，files 数组大时渲染开销可观
 *  dynamic 的 chunk 落在懒加载包，收起态零下载；loading 骨架复用
 *  SkeletonRows（34px 行高与真实表格对齐，防展开跳动）。 */
export const TorrentManageLazy = dynamic(
  () => import("@/components/torrent-manage").then((m) => m.TorrentManage),
  { ssr: false, loading: () => <SkeletonRows rows={6} /> },
);

export const TorrentPeersLazy = dynamic(
  () => import("@/components/torrent-peers").then((m) => m.TorrentPeers),
  { ssr: false, loading: () => <SkeletonRows rows={6} /> },
);

export const FileTreeLazy = dynamic(
  () => import("@/components/file-tree").then((m) => m.FileTree),
  { ssr: false, loading: () => <SkeletonRows rows={8} /> },
);

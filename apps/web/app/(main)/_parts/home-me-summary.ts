/** 首页移动数据条（M3）四格文本。
 *
 * 口径只有一条：字节走 `formatBytes`、分享率走 `formatRatio`，与电脑端右上
 * 角（`components/user-menu.tsx` 两处）读的是同一支函数。
 *
 * 此前这里自己写 `${bytes / 1e9}G`：按 1000³ 折算却标着 G，与站内其余十几
 * 处按 1024³ 的读法分叉——同一账号 5 GiB 上传，电脑显示「5.0 GB」、手机显示
 * 「5.4G」（多报 7.4%）；下载 30 MiB 显示「0.0G」，即「明明下载过，屏上是没有」。
 */
import { formatBytes, formatRatio } from "@/lib/format";

/** `/me/overview` 里首页要用的三个数（缺字段按 0 计，与旧口径一致） */
export interface HomeMeRaw {
  uploaded?: number;
  downloaded?: number;
  spark_balance?: number;
}

export interface HomeMeSummary {
  uploaded: string;
  downloaded: string;
  ratio: string;
  spark: string;
}

export function homeMeSummary(
  ov: HomeMeRaw | null,
  locale: string,
): HomeMeSummary | null {
  if (!ov) return null;
  const up = ov.uploaded ?? 0;
  const down = ov.downloaded ?? 0;
  return {
    uploaded: formatBytes(up),
    downloaded: formatBytes(down),
    ratio: formatRatio(up, down),
    spark: (ov.spark_balance ?? 0).toLocaleString(locale),
  };
}

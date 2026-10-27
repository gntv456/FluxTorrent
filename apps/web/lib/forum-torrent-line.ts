/** 种子引用行识别（渲染层唯一口径，forum-markdown 块解析调用）：
 *  !torrent(id) 显式语法，或整行恰为站内种子链接（/torrent/{id} 相对
 *  路径 / 任意绝对 URL 且 pathname 恰为 /torrent/{id}，尾斜杠容忍）。
 *  返回种子 id 或 null。解析器里须在 videoLine 之前判，否则裸链会被
 *  视频候选先抢走。
 *
 *  独立中性模块（非 "use client"）：调用方 forum-markdown 是 server
 *  component，从 client 模块导入并在服务端调用会被 Next 判为「在服务端
 *  调用客户端函数」直接抛错——整页落错误边界。
 */
export function torrentLine(line: string): number | null {
  const t = line.trim();
  const tm = t.match(/^!torrent\((\d{1,10})\)$/);
  if (tm) return Number(tm[1]);
  let u: URL | null = null;
  try {
    u = /^https?:\/\//i.test(t) ? new URL(t) : null;
  } catch {
    u = null;
  }
  const path = u ? u.pathname : t.startsWith("/") ? t : null;
  const m = path?.match(/^\/torrent\/(\d{1,10})\/?$/);
  return m ? Number(m[1]) : null;
}

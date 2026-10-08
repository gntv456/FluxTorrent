// API 上游地址单源（硬编码审计 P2 收敛）：
// 此前 `process.env.API_SERVER_URL ?? "http://localhost:8080"` 在
// api-client 与 route handler 各写一份（next.config rewrites 还有第三处），
// 改端口要动三个文件。统一从这里导出。
//
// 语义：「服务端直连 api 服务的基址」。浏览器侧另有
// NEXT_PUBLIC_API_URL ?? ""（同源相对路径）口径，见 api-client.ts。

/** 服务端（RSC / route handler / 容器内）直连 api 服务的基址 */
export function apiServerBase(): string {
  return process.env.API_SERVER_URL ?? "http://localhost:8080";
}

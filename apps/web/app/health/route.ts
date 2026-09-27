import { NextResponse } from "next/server";

/**
 * web 健康检查端点（0224 G30-A1）：给 LB / compose healthcheck 判活用。
 * 纯静态 200，不探 api/DB 依赖——web 自身进程活着就该绿（依赖故障由
 * api 容器自己的 healthcheck 表达，两层不混）。
 */
export const dynamic = "force-static";

export async function GET() {
  return NextResponse.json({ ok: true });
}

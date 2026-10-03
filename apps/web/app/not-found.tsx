import Link from "next/link";

import { EmptyState } from "@/components/ui/empty-state";

/** 404 页（2026-10-03 新建）。
 *
 * 为什么需要：此前全站没有 not-found.tsx，所有 404 落在 Next.js
 * 内置错误页——它硬编码 `body{color:#000;background:#fff;margin:0}`
 * 并以 `<style dangerouslySetInnerHTML>` 注入，夜间模式下是一整页
 * 刺眼白底（实测 200 万 px² 发白，扫描器最大误报源），且没有站点
 * 顶栏与导航，用户被丢在荒岛上。
 *
 * 自定义后 Next 不再注入内置样式；页面走全站令牌（--surface-* /
 * --text-*），浅色/夜间自动跟随。空态三件套（图标+文案+引导动作）
 * 与 components/ui/empty-state.tsx 的设计规格一致。
 *
 * 注意：404 可能发生在登录前（错的分享链接），故不假设登录态，
 * 只给「返回首页」这个永远安全的出口。
 */
export default function NotFound() {
  return (
    <main className="grid min-h-[60vh] place-items-center px-4">
      <div className="w-full max-w-md">
        <EmptyState
          icon="search"
          title="404 · 页面不存在"
          desc="链接可能已失效，或资源已被删除。种子页地址形如 /torrent/{id}，可回首页搜索关键词找到同类资源。"
          action={
            <Link href="/" className="ui-btn ui-btn--primary">
              返回首页
            </Link>
          }
        />
      </div>
    </main>
  );
}

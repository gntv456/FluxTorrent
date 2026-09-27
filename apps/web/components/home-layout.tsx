/** 首页排版（0089 / 四审 L6 单源化）——**兼容转发层**。
 *
 *  实现已迁至 `home-layout-parse.ts`（纯函数、无 "use client"），以便
 *  服务端 RSC 与客户端组件共用同一份解析逻辑（否则 RSC 调用会报
 *  「Attempted to call a client function from the server」）。
 *  本文件保留为转发入口，历史 import 路径 `@/components/home-layout` 不受影响。 */

export {
  defaultLayout,
  parseHomeLayout,
  effectiveSpan,
  type HomeLayoutItem,
  type HomeSectionMeta,
} from "@/components/home-layout-parse";

import type { IconName } from "@/components/icons";

/**
 * 字符符号 → Icon 名映射表（S1 快速修 2026-10-16）。
 *
 * 背景：全站多处用键盘字符/装饰符号充当图标（⚙ ◷ ⬇ ★ ✎ ✓ ☰ ✦ ♥ →），
 * 字符在不同平台缺字形时会渲染成豆腐块或错字（usercp 的 ✎ 曾在多数字体下
 * 被渲染成「、」，见 usercp.tsx 2026-10-03 记录），且无法着色、不随主题换色。
 *
 * 口径（重要，避免误伤）：
 *  - 只替换「装饰性图标位」——原本就 aria-hidden 包裹、或独立成图标的符号；
 *  - 不替换文案里的符号：如状态流转「open → drawn」、评分「★ 8.5」、
 *    下载数「⬇️ 12」，它们是内容而非图标；
 *  - 暂无对应图标的符号不上表（→ arrow / ☰ menu / ✦ / ♥ heart），保留字符，
 *    待图标补齐后再收敛。
 *
 * 本表是数据（供替换与后续门禁参考）；已替换的实例见 usercp、usercp-overview
 * 与 users/[id] 的 profile-hero 等组件。独立成文件以避开 icons.tsx 的行数基线。
 */
export const SYMBOL_ICON: Record<string, IconName> = {
  "⚙": "settings",
  "◷": "clock",
  "⬇": "download",
  "★": "star",
  "✎": "edit",
  "✓": "checkCircle",
};

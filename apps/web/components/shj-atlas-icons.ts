/** 山海图鉴侧栏图标表（从 shj-atlas.tsx 按 300 行门禁拆出）。
 *  href → 图标（emoji，对齐设计样图 V1.2 图鉴图标位）。46 项逐项字面量
 *  会挤爆行数门禁，存成空白分隔的「路径 表情」对，启动期解析成 Map。 */
const ICON_TOKS = `
/ 🏠 /torrents 📚 /forums 💬 /upload 📤 /top 🏆 /requests 🙏 /offers 🎁
/networks 📡 /artists 🎤 /seasonal 🌸 /subtitles 📑 /subtitle-groups 👥
/preserve 🛡️ /endangered 🆘 /teams 🤝 /textbooks 📖 /shop 🛒 /bank 🏦
/magic-pool 🔮 /tasks ✅ /my-spark ✨ /me/sparks 📒 /classes 🎓 /me/exams 📝
/me/achievements 🏅 /resurrections 💫 /medal-wall 🎖️ /avatar-frames 🖼️
/medals 🏵️ /jixiao 📊 /invites ✉️ /games 🎮 /gacha 🥚 /farm 🌾 /gomoku ⚫
/contests 🏁 /friends 🧑‍🤝‍🧑 /messages 📨 /myhr ⚖️ /me/subscriptions 📺
/faq ❓ /clients 🧭 /wiki 📔 /donate 💝
`
  .split(/\s+/)
  .filter(Boolean);
const ICONS = new Map<string, string>();
for (let i = 0; i < ICON_TOKS.length; i += 2) {
  ICONS.set(ICON_TOKS[i], ICON_TOKS[i + 1]);
}

export function ico(href: string): string {
  const path = href.split("?")[0];
  if (href.includes("official=1")) return "🏅";
  // 教育资源与资源库同路径不同 query：按教育分类特判，避免两处都显示 📚
  if (href.includes("category_id=13")) return "📖";
  return ICONS.get(path) ?? "🔹";
}

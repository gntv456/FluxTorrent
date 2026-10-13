// ui_design_guard 规则 9 独立模块（W-44）：L4→L1 禁令——tsx 六位 hex
// 只减不增。抽出主脚本是为过 300 行门禁（同 ui_design_guard_dom.mjs 先例）。
//
// DS-PLAN §2 分层宪法：L4 页面层禁止直用 L1 令牌层的原始形态（hex
// 字面量）。口径与 design_cov_guard 指标 3 对齐（豁免域/行级豁免照抄），
// 但本规则是「存在即违规」的棘轮（存量登记进 baseline，新增一处即
// 挂），比 design_cov 的「总数不涨」更严——防新增硬编码色。
import path from "node:path";

export function checkTsxHex(ctx) {
  const { section, failBox, warnBox, read, base, UPDATE, globSync, ROOT } = ctx;
  section("9. L4→L1：tsx 硬编码 hex（只减不增）");
  // games 豁免域与 design_cov_guard 同源（DS §6：SVG 皮肤是内容不是
  // chrome）；components/ui/ 不纳入（原语自身在令牌层侧，样式走
  // ui.css；纳入是另一次收紧，不在 W-44 范围）。
  const EXEMPT_TSX = [
    "apps/web/components/game/",
    "apps/web/app/(main)/games/",
    "apps/web/app/(main)/gacha/",
  ];
  // 行级豁免照抄 design_cov（design-cov-exempt：平台强制字面量，
  // 如 viewport themeColor / manifest background_color，须带原因注释）
  const EXEMPT_MARK = "design-cov-exempt";
  const files = globSync("apps/web/{app,components}/**/*.tsx", { cwd: ROOT })
    .map((f) => f.replace(/\\/g, "/"))
    .filter((f) => !EXEMPT_TSX.some((p) => f.startsWith(p)));
  let total = 0;
  const perFile = {};
  for (const f of files) {
    const lines = read(f).split("\n");
    let n = 0;
    lines.forEach((l, i) => {
      const prev = i > 0 ? lines[i - 1] : "";
      if (l.includes(EXEMPT_MARK) || prev.includes(EXEMPT_MARK)) return;
      n += (l.match(/#[0-9a-fA-F]{6}\b/g) ?? []).length;
    });
    if (n > 0) {
      perFile[f] = n;
      total += n;
    }
  }
  const prev = base.tsxHex ?? total;
  if (total > prev) {
    failBox.n++;
    console.error(`  FAIL：${total} 处（基线 ${prev}，增加 ${total - prev}）`);
    for (const [f, n] of Object.entries(perFile).sort((a, b) => b[1] - a[1]))
      console.error(`    ${n}\t${f}`);
    console.error(
      "    → 换语义令牌 var(--*) / tone class；平台字面量加" +
        " design-cov-exempt 行级豁免（带原因注释）",
    );
  } else if (total < prev) {
    warnBox.n++;
    console.log(`  OK：${total} 处（基线 ${prev}，减少 ${prev - total}）`);
  } else {
    console.log(`  OK：${total} 处（持平）`);
  }
  base.tsxHex = UPDATE ? total : prev;
}

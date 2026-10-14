// ui_design_guard 规则 10 独立模块（D5 决议 2026-10-14，按建议 B）：动效
// 时长棘轮——transition 带时长却未引用动效令牌，只许降。抽出主脚本是为
// 过 300 行门禁（同 ui_design_guard_dom.mjs / _hex.mjs 先例）。
//
// 判据：声明含时长（s/ms）但既无 `var(--motion-*)`（base.css，带
// reduced-motion 全局降级）也无 `var(--tide-*)`（theme-tide.css 缓动族）。
// `transition: none` 是「禁用动画」语义，不属时长选择，不计。
// D5 决议：存量**触碰即迁**（不本期清偿），但棘轮「只许降不许增」——
// 新代码写时长必须走令牌。

export function checkMotionDurations(ctx) {
  const { section, failBox, warnBox, read, base, UPDATE } = ctx;
  section("10. 动效时长散落（transition 带时长却未引用动效令牌，只许降）");
  const DUR = /\d+(?:\.\d+)?m?s\b/;
  const per = new Map();
  for (const f of ctx.allCss) {
    const s = read(f);
    const decls = s.match(/transition[^;{]*;/g) ?? [];
    const n = decls.filter(
      (d) =>
        DUR.test(d) && !d.includes("var(--motion") && !d.includes("var(--tide"),
    ).length;
    if (n > 0) per.set(f, n);
  }
  const mt = [...per.values()].reduce((a, b) => a + b, 0);
  const prev = base.scatteredMotionDurations ?? mt;
  if (mt > prev) {
    failBox.n++;
    console.error(
      `  FAIL：${mt} 处散落时长（基线 ${prev}，增加 ${mt - prev}）`,
    );
    for (const [f, n] of per) console.error(`    ${n}\t${f}`);
    console.error(
      "    → 时长/缓动用 var(--motion-fast|normal|slow) / var(--tide-ease*)",
    );
  } else if (mt < prev) {
    warnBox.n++;
    console.log(`  OK：${mt} 处散落时长（基线 ${prev}，减少 ${prev - mt}）`);
  } else {
    console.log(`  OK：${mt} 处散落时长（基线 ${prev}）`);
  }
  base.scatteredMotionDurations = UPDATE ? mt : prev;
}

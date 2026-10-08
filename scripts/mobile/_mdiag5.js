export default async (h) => {
  // torrents 页 tsb-adv__summary 为什么有溢出 (gear 模式下 summary 应是 1px 隐藏)
  await h.navigate('http://localhost:3000/torrents');
  const r = await h.evalPage(`(() => {
    const sum = document.querySelector('summary.tsb-adv__summary');
    if (!sum) return { found: false };
    const cs = getComputedStyle(sum);
    const parent = sum.parentElement;
    return {
      found: true,
      pos: cs.position, w: cs.width, clip: cs.clipPath,
      parentCls: String(parent.className).slice(0, 40),
      overflowDiff: sum.scrollWidth - sum.clientWidth,
      // 它在文档里占据的盒子
      rect: { w: Math.round(sum.getBoundingClientRect().width), x: Math.round(sum.getBoundingClientRect().x) },
    };
  })()`);
  console.log(JSON.stringify(r, null, 1));
};

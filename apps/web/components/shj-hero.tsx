/** 山海经首页 Hero「山海图卷」（乙 · 结构层）。
 *
 *  纯展示组件（无交互、无 hooks，保持 server component 不进客户端包）。
 *  恒渲染、CSS 控显隐：仅 [data-theme="shanhaijing"] 下出现，其余三主题
 *  display:none、零布局影响（同 ShjAtlas 纪律）。
 *  站点数据玉牌来自 /home.site_data 同源计数，数据缺失时只落题字不落牌。 */
export function ShjHero({
  brand,
  themeName,
  kicker,
  sub,
  jades,
}: {
  brand: string;
  /** 竖排卷签文字（主题名，随语言） */
  themeName: string;
  kicker: string;
  sub: string;
  /** 玉牌：{ label, value } ×4；空数组 = 数据未取到，不渲染牌区 */
  jades: { label: string; value: string }[];
}) {
  return (
    <section className="shj-hero" aria-label={kicker}>
      <span className="shj-hero__vtag" aria-hidden>
        {themeName}
      </span>
      <div className="shj-hero__inner">
        <p className="shj-hero__kicker">{kicker}</p>
        <h2 className="shj-hero__title">{brand}</h2>
        <p className="shj-hero__sub">{sub}</p>
        {jades.length > 0 && (
          <ul className="shj-hero__jades">
            {jades.map((j) => (
              <li key={j.label} className="shj-hero__jade">
                <b>{j.value}</b>
                <i>{j.label}</i>
              </li>
            ))}
          </ul>
        )}
      </div>
      <span className="shj-hero__vtag shj-hero__vtag--r" aria-hidden>
        {themeName}
      </span>
    </section>
  );
}

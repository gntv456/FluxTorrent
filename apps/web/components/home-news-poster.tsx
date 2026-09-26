/** 首页公告卡海报位（M4 桌面打磨 #4）：纯品牌字占位密度低——
 *  叠加公告标题列表填充整卡；配真海报后还原纯图（外部条件控制）。 */
export function HomeNewsPoster({
  brand,
  items,
}: {
  brand: string;
  items: { id: number; title: string }[];
}) {
  return (
    <div className="home-news__poster" aria-hidden="true">
      <span className="home-news__poster-brand">{brand}</span>
      <ul className="home-news__poster-list">
        {items.map((n) => (
          <li key={n.id}>{n.title}</li>
        ))}
      </ul>
    </div>
  );
}

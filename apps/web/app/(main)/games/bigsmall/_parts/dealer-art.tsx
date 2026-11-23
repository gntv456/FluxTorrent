/**
 * 荷官立绘：舞台前景人物（透明底 webp，public/games/）。
 * 纯展示件，不参与结算；高度/宽度由舞台 CSS 断点控制，保持原始比例。
 */
export function DealerArt() {
  return (
    <div className="bs-dealer" aria-hidden>
      <img
        className="bs-dealer-img"
        src="/games/dealer-bigsmall.webp"
        alt=""
        draggable={false}
      />
    </div>
  );
}

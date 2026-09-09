/** 离线回退页（M26：SW 导航失败时的兜底） */
export default function OfflinePage() {
  return (
    <div className="mx-auto flex max-w-sm flex-col items-center gap-4 py-16 text-center">
      <span aria-hidden className="text-[72px] leading-none">
        🦉💤
      </span>
      <h1 className="font-display text-2xl">猫头鹰打了个盹</h1>
      <p className="text-sm text-sub">
        当前网络不可用。看看已缓存的内容，或稍后再试试～
      </p>
      <a
        href="/"
        className="min-h-[44px] inline-flex items-center rounded-full bg-sky px-6 font-bold text-white"
      >
        回首页
      </a>
    </div>
  );
}

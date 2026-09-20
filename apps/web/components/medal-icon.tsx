/**
 * 勋章图标：`asset_ref` 有值时画图（与头像 / 头像框 / 道具图片同口径——存 URL，不存文件），
 * 否则回落 🏅 字形。全站勋章展示位（勋章殿堂 / 勋章墙 / 用户详情佩戴勋章 / 用户名角标）共用，
 * 避免各写一份 img-or-emoji 分支。
 */
export function MedalIcon({
  src,
  size = 28,
  className = "",
  title,
}: {
  src?: string | null;
  size?: number;
  className?: string;
  title?: string;
}) {
  const url = usableAssetUrl(src);
  if (!url) {
    return (
      <span
        aria-hidden
        className={`inline-flex items-center justify-center ${className}`}
        style={{ width: size, height: size, fontSize: size * 0.82, lineHeight: 1 }}
      >
        🏅
      </span>
    );
  }
  return (
    // eslint-disable-next-line @next/next/no-img-element
    <img
      src={url}
      alt={title ?? ""}
      title={title}
      loading="lazy"
      className={`inline-block shrink-0 rounded-[6px] object-cover ${className}`}
      style={{ width: size, height: size }}
    />
  );
}

/** 只放行 http(s) 绝对地址与站内根路径：避免 javascript:/data: 之类塞进 img src。 */
export function usableAssetUrl(ref?: string | null): string | null {
  const t = (ref ?? "").trim();
  if (!t) return null;
  return /^https?:\/\//i.test(t) || t.startsWith("/") ? t : null;
}

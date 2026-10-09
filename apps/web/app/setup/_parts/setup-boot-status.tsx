"use client";

import type { SetupStatusState } from "./use-setup-status";

/**
 * 向导首屏的加载态 / 失败诊断块（从 setup/page.tsx 按域拆出，300 行门禁）。
 *
 * 存在的理由：旧实现 `catch(() => setError("无法加载向导状态…"))` 把所有
 * 失败压成一句话、且不重试。api 先跑完迁移才 bind 端口，全新装机时
 * /setup 必然落进失败分支——站长看到一句无法行动的话，只能手动刷页面。
 * 这里把「主文案 + 技术细节 + 处置建议 + 手动重试」四件事摆出来，
 * 让冷启动可自愈、真实故障可定位。
 */

export function SetupBootStatus({
  boot,
  t,
}: {
  boot: SetupStatusState;
  t: (k: string, fallback: string) => string;
}) {
  // 首次探测中：不报错，只提示正在加载
  if (!boot.error) {
    if (boot.status) return null;
    return (
      <p
        className="mt-4 rounded-md border border-line
          bg-[var(--panel)] px-3 py-2 text-sm text-muted"
      >
        {boot.probing
          ? t("stillWaiting", "API 尚未就绪，正在自动重试…")
          : t("loadingStatus", "正在加载向导状态…")}
      </p>
    );
  }

  return (
    <div
      className="mt-4 rounded-md border border-danger/40 bg-danger/10
        px-3 py-2 text-sm text-danger"
    >
      <p className="font-medium">{boot.error}</p>
      {boot.detail && (
        <p className="mt-1 font-mono text-xs break-all opacity-80">
          {boot.detail}
        </p>
      )}
      {boot.hint && <p className="mt-1 text-xs opacity-90">{boot.hint}</p>}
      <div className="mt-2 flex items-center gap-3">
        <button
          type="button"
          onClick={boot.reload}
          disabled={boot.probing}
          className="rounded border border-danger/40 px-2 py-1 text-xs
            transition-colors hover:bg-danger/20 disabled:opacity-50"
        >
          {boot.probing
            ? t("retrying", "重试中…")
            : t("retryLoad", "重新加载")}
        </button>
        {boot.probing && (
          <span className="text-xs opacity-70">
            {t("attempt", "第 {n} 次探测").replace(
              "{n}",
              String(boot.attempt),
            )}
          </span>
        )}
      </div>
    </div>
  );
}

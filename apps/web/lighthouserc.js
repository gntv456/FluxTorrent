// Lighthouse CI 配置（DS-PLAN-01 W-08：P0 十页轮换 + 性能预算门禁）。
//
// 性能预算（DS §12.1，Lighthouse 移动档）：
//   LCP ≤ 2500ms / CLS ≤ 0.1 / TBT ≤ 300ms（INP 是现场指标，实验室
//   用 TBT 代理；真值需真实用户数据，见 docs/design/design-system-plan.md）。
//
// 运行（本地栈已在跑；与 playwright.config.ts 同前置假设）：
//   pnpm lhci                          # P0 十页全量（游客态）
//   LHC_LOGIN=1 LHC_USER=root LHC_PASS=*** pnpm lhci   # 含登录态页
//   CHROME_PATH=/path/to/chrome pnpm lhci              # 指定浏览器
//
// 登录态实现（2026-10-13 实证，两个失败方案的教训写在
// e2e/lhci-login-token.cjs 头部）：登录 API 取 JWT → Lighthouse
// settings.extraHeaders 经 CDP 给所有请求戴 Cookie
// （flux_token=JWT + flux.session=1，后者是 Next middleware 的页面
// 准入标记）。 puppeteerScript/预置 profile 两条路都走不通。
//
// Chrome 解析优先级：CHROME_PATH → PLAYWRIGHT_BROWSERS_PATH（默认
// ~/AppData/Local/ms-playwright 或 ~/.cache/ms-playwright）。本仓已装
// @playwright/test，其 Chromium 直接复用，不为 Lighthouse 再下一份。
const fs = require("node:fs");
const path = require("node:path");
const { execFileSync } = require("node:child_process");

function resolveChromePath() {
  if (process.env.CHROME_PATH && fs.existsSync(process.env.CHROME_PATH)) {
    return process.env.CHROME_PATH;
  }
  const bases = [];
  if (process.env.PLAYWRIGHT_BROWSERS_PATH) {
    bases.push(process.env.PLAYWRIGHT_BROWSERS_PATH);
  }
  const home = process.env.USERPROFILE || process.env.HOME;
  if (home) {
    bases.push(path.join(home, "AppData", "Local", "ms-playwright"));
    bases.push(path.join(home, ".cache", "ms-playwright"));
  }
  for (const base of bases) {
    if (!fs.existsSync(base)) continue;
    let dirs = [];
    try {
      dirs = fs.readdirSync(base);
    } catch {
      continue;
    }
    for (const d of dirs.sort().reverse()) {
      if (!/^chromium-\d+$/.test(d)) continue;
      for (const exe of [
        path.join(base, d, "chrome-win64", "chrome.exe"),
        path.join(base, d, "chrome-win", "chrome.exe"),
        path.join(base, d, "chrome-linux", "chrome"),
        path.join(
          base,
          d,
          "chrome-mac",
          "Chromium.app",
          "Contents",
          "MacOS",
          "Chromium",
        ),
      ]) {
        if (fs.existsSync(exe)) return exe;
      }
    }
  }
  return undefined;
}

/** LHC_LOGIN=1 时取 JWT 并构造 extraHeaders（同步：配置加载期执行） */
function authSettings() {
  if (process.env.LHC_LOGIN !== "1") return {};
  if (!process.env.LHC_PASS) {
    console.warn(
      "[lighthouserc] LHC_LOGIN=1 但未设 LHC_PASS，按游客态采集",
    );
    return {};
  }
  const token = execFileSync(
    process.execPath,
    [path.join(__dirname, "e2e", "lhci-login-token.cjs")],
    { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] },
  ).trim();
  if (!token) {
    console.warn("[lighthouserc] 取 token 失败，按游客态采集");
    return {};
  }
  return {
    // 双 cookie：API 鉴权 + middleware 页面准入（middleware.ts:54）
    extraHeaders: {
      Cookie: `flux_token=${token}; flux.session=1`,
    },
    // 不重置存储：cookie 由 extraHeaders 每请求重戴，双保险
    disableStorageReset: true,
  };
}

const chromePath = resolveChromePath();
const auth = authSettings();

module.exports = {
  ci: {
    collect: {
      // 游客态可达页（C 批 2026-10-14 修正：原 P0 十页在游客态全被
      // middleware.ts 307 到 /login，测的全是登录页——游客态只测真正
      // 可达的公开页，curl -I 逐一验过 200：/login /register /faq
      // /rules /ban-log /appeals /forgot /reset /offline /setup。
      // /about 不存在（307），勿加。
      // 登录态十页（torrents/torrent/40/upload/messages/forums/
      // forums/topic/13/torrents?search=test/users/1/top/notifications）
      // 待 LHC_PASS 采集：LHC_LOGIN=1 LHC_USER=root LHC_PASS=*** pnpm lhci
      url: [
        "http://localhost:3000/login",
        "http://localhost:3000/register",
        "http://localhost:3000/faq",
        "http://localhost:3000/rules",
        "http://localhost:3000/ban-log",
        "http://localhost:3000/appeals",
        "http://localhost:3000/forgot",
        "http://localhost:3000/reset",
        "http://localhost:3000/offline",
        "http://localhost:3000/setup",
      ],
      numberOfRuns: 1,
      ...(chromePath ? { chromePath } : {}),
      settings: {
        // 移动档 Slow 4G（preset:"mobile" 需新版 Lighthouse，0.14 内置
        // 12.1 不认，显式给节流参数等价）
        emulatedFormFactor: "mobile",
        onlyCategories: ["performance"],
        throttling: {
          rttMs: 150,
          throughputKbps: 1638.4,
          cpuSlowdownMultiplier: 4,
        },
        ...auth,
      },
    },
    assert: {
      assertions: {
        "largest-contentful-paint": ["error", { maxNumericValue: 2500 }],
        "cumulative-layout-shift": ["error", { maxNumericValue: 0.1 }],
        "total-blocking-time": ["error", { maxNumericValue: 300 }],
      },
    },
    upload: {
      target: "filesystem",
      outputDir: ".lighthouseci",
      reportFilenamePattern: "%%PATHNAME%%-%%DATETIME%%.report.html",
    },
  },
};

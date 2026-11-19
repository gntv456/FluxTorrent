// FluxTorrent Service Worker（M26）
// 策略（§5.7 离线策略）：静态资源缓存优先、页面导航网络优先失败回退、API 永不缓存。
const VERSION = "flux-v4";
const STATIC_CACHE = `${VERSION}-static`;
const PAGE_CACHE = `${VERSION}-pages`;
const OFFLINE_URL = "/offline";
const PRECACHE = [
  "/manifest.webmanifest",
  "/icons/icon-192.png",
  "/icons/icon-512.png",
  OFFLINE_URL,
];

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches
      .open(STATIC_CACHE)
      .then((c) => Promise.allSettled(PRECACHE.map((u) => c.add(u))))
      .then(() => self.skipWaiting()),
  );
});

// 页面注册脚本发 SKIP_WAITING：新 SW 立即接管（配合页面刷新一次拿新 bundle）
self.addEventListener("message", (event) => {
  if (event.data === "SKIP_WAITING") self.skipWaiting();
  // 三轮遗留（2026-10-07）：登出清页面缓存——PAGE_CACHE 里存着登录态
  // RSC HTML（含用户名），共用设备离线态可回看上次用户的页面快照。
  if (event.data === "CLEAR_PAGES") {
    event.waitUntil(caches.delete(PAGE_CACHE));
  }
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(
          keys
            .filter((k) => !k.startsWith(VERSION))
            .map((k) => caches.delete(k)),
        ),
      )
      .then(() => self.clients.claim())
      .then(() =>
        self.clients.matchAll({ type: "window" }).then((clients) => {
          clients.forEach((c) => c.postMessage("FLUX_SW_ACTIVATED"));
        }),
      ),
  );
});

self.addEventListener("fetch", (event) => {
  const url = new URL(event.request.url);
  if (event.request.method !== "GET") return;
  // 同源才接管；API 与 tracker 流量永不缓存（计费/权限敏感）
  if (url.origin !== self.location.origin) return;
  if (url.pathname.startsWith("/api/")) return;

  // Next.js 构建产物（/_next/static）：哈希不变，缓存优先
  if (url.pathname.startsWith("/_next/static/")) {
    event.respondWith(
      caches.match(event.request).then(
        (hit) =>
          hit ??
          fetch(event.request).then((res) => {
            const copy = res.clone();
            caches.open(STATIC_CACHE).then((c) => c.put(event.request, copy));
            return res;
          }),
      ),
    );
    return;
  }

  // 页面导航：网络优先，离线回退缓存，再回退离线页
  if (event.request.mode === "navigate") {
    event.respondWith(
      fetch(event.request)
        .then((res) => {
          const copy = res.clone();
          caches.open(PAGE_CACHE).then((c) => c.put(event.request, copy));
          return res;
        })
        .catch(() =>
          caches
            .match(event.request)
            .then((hit) => hit ?? caches.match(OFFLINE_URL)),
        ),
    );
  }
});


// 0503 Push 展示链补齐：订阅侧（push-settings.tsx 走 pushManager）一直
// 完整，但 SW 侧无人消费 push 事件——后台通知静默丢，三类偏好形同虚设。
// 载荷口径：后端 push_outbox 发 { title, body, url?, tag? }（冗余兼容
// data.{title,body} 形态）；tag 去重防同题材连发叠屏。
self.addEventListener("push", (event) => {
  let p = {};
  try {
    p = event.data ? event.data.json() : {};
  } catch (e) {
    p = { body: event.data ? event.data.text() : "" };
  }
  const d = p.data || p;
  const title = d.title || p.title || "FluxTorrent";
  const body = d.body || p.body || "";
  if (!body) return;
  event.waitUntil(
    self.registration.showNotification(title, {
      body,
      tag: d.tag || "flux",
      icon: "/icons/icon-192.png",
      badge: "/icons/icon-192.png",
      data: { url: d.url || "/" },
    }),
  );
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const url = (event.notification.data && event.notification.data.url) || "/";
  event.waitUntil(
    self.clients.matchAll({ type: "window", includeUncontrolled: true })
      .then((cs) => {
        for (const c of cs) {
          if ("focus" in c) {
            c.navigate(url);
            return c.focus();
          }
        }
        return self.clients.openWindow(url);
      }),
  );
});

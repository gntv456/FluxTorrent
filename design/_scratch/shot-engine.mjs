import { chromium } from "file:///D:/FluxTorrent/node_modules/.pnpm/playwright@1.63.0/node_modules/playwright/index.mjs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const shots = path.resolve(here, "..", "_shots");
const BASE = "http://127.0.0.1:3001/games/scratch";
const EDGE = "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe";
const browser = await chromium.launch({ executablePath: EDGE, headless: true });

const cases = [
  { tag: "engine-1440", vp: { width: 1440, height: 900 } },
  { tag: "engine-768", vp: { width: 768, height: 1024 } },
  { tag: "engine-390", vp: { width: 390, height: 844 } },
];

for (const c of cases) {
  const ctx = await browser.newContext({ viewport: c.vp, deviceScaleFactor: 1 });
  const page = await ctx.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("console", (m) => {
    if (m.type() === "error") errors.push(m.text());
  });
  await page.goto(BASE, { waitUntil: "domcontentloaded" });
  await page.waitForTimeout(1000);
  await page.screenshot({ path: path.join(shots, `${c.tag}.png`), fullPage: true });
  console.log(c.tag, "errors:", errors.length ? errors : "none");
  await ctx.close();
}

await browser.close();
console.log("done ->", shots);

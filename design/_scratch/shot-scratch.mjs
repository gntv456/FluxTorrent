import { chromium } from "file:///D:/FluxTorrent/node_modules/.pnpm/playwright@1.63.0/node_modules/playwright/index.mjs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const shots = path.resolve(here, "..", "_shots");
const html = path.resolve(here, "..", "刮刮乐页面样图.html");
const url = pathToFileURL(html).href;

const EDGE = "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe";
const browser = await chromium.launch({ executablePath: EDGE, headless: true });

const cases = [
  { tag: "1440", vp: { width: 1440, height: 900 }, tabs: ["play", "pool", "records"] },
  { tag: "768",  vp: { width: 768,  height: 1024 }, tabs: ["play"] },
  { tag: "390",  vp: { width: 390,  height: 844 },  tabs: ["play", "pool", "records"] },
];

for (const c of cases) {
  const ctx = await browser.newContext({ viewport: c.vp, deviceScaleFactor: 1 });
  const page = await ctx.newPage();
  const errors = [];
  page.on("pageerror", e => errors.push(e.message));
  await page.goto(url, { waitUntil: "domcontentloaded" });
  await page.waitForTimeout(600);

  for (const tab of c.tabs) {
    await page.click(`.gs-tab[data-tab="${tab}"]`);
    await page.waitForTimeout(300);
    await page.screenshot({ path: path.join(shots, `scratch-${c.tag}-${tab}.png`), fullPage: true });
  }
  console.log(c.tag, "errors:", errors.length ? errors : "none");
  await ctx.close();
}

await browser.close();
console.log("done ->", shots);

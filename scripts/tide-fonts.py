"""TIDE 字体自托管：拉取 Google Fonts 的 CSS（含 unicode-range 分片），
把其中所有 woff2 下载到 apps/web/public/fonts/，并把 CSS 里的 URL 改写为本地路径。
浏览器仍按需只下载「渲染到对应字符」的分片，所以首访体积不变，但彻底摆脱外网依赖。"""

import pathlib
import re
import urllib.request

UA = (
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
    "(KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
)
OUT_DIR = pathlib.Path(r"D:\FluxTorrent\apps\web\public\fonts")
CSS_OUT = pathlib.Path(r"D:\FluxTorrent\apps\web\app\styles\fonts-tide.css")

# 只取实际用到的字重：display 400（h1 默认）+ 700（加粗/板块标题）、mono 400/500
FAMILIES = [
    ("Bricolage+Grotesque:opsz,wght@12..96,400..700", "bricolage"),
    ("IBM+Plex+Mono:wght@400;500", "plexmono"),
    ("Noto+Serif+SC:wght@400;700", "notoserifsc"),
]


def fetch(url: str) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=60) as resp:
        return resp.read()


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    blocks: list[str] = []
    total_files = 0
    total_bytes = 0

    for spec, slug in FAMILIES:
        css_url = (
            f"https://fonts.googleapis.com/css2?family={spec}&display=swap"
        )
        css = fetch(css_url).decode("utf-8")
        urls = sorted(set(re.findall(r"url\((https://[^)]+\.woff2)\)", css)))
        print(f"[{slug}] CSS 分片数 = {len(urls)}", flush=True)

        mapping: dict[str, str] = {}
        for i, full in enumerate(urls, 1):
            name = f"{slug}-{full.rsplit('/', 1)[-1]}"
            dest = OUT_DIR / name
            if not dest.exists():
                data = fetch(full)
                dest.write_bytes(data)
                total_bytes += len(data)
            mapping[full] = name
            total_files += 1
            if i % 25 == 0:
                print(f"  ...{i}/{len(urls)}", flush=True)

        for full, name in mapping.items():
            css = css.replace(full, f"/fonts/{name}")
        blocks.append(f"/* ===== {slug}（自托管分片）===== */\n{css.strip()}\n")

    header = (
        "/* TIDE 字体自托管（由 _tide_fonts.py 生成，勿手改）\n"
        "   来源：Google Fonts（Bricolage Grotesque / IBM Plex Mono / Noto Serif SC）\n"
        "   说明：保留原始 unicode-range 分片，浏览器按需加载；文件在 public/fonts/。\n"
        "   更新方式：改 _tide_fonts.py 的 FAMILIES 后重跑。 */\n\n"
    )
    CSS_OUT.write_text(header + "\n".join(blocks), encoding="utf-8")
    mb = total_bytes / 1024 / 1024
    print(f"\n完成：{total_files} 个文件，本轮新增下载 {mb:.2f} MB", flush=True)
    print(f"CSS 写入 {CSS_OUT}", flush=True)


if __name__ == "__main__":
    main()

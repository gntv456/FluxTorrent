"""山海经字体自托管：拉取 Google Fonts 的 CSS（含 unicode-range 分片），
把其中所有 woff2 下载到 apps/web/public/fonts/，并把 CSS 里的 URL 改写为本地路径。
机制与 tide-fonts.py 相同：浏览器按需只下载「渲染到对应字符」的分片。

只含山海经新增的两族（Ma Shan Zheng 题字 / ZCOOL XiaoWei 签条）；
Noto Serif SC 已由 tide-fonts.py 托管（fonts-tide.css 注册了 400/700），
globals.css 里两条 @import 都在，族名相同即复用，不重复下载。
注意：tide 托管的 Noto Serif SC 只有 400/700 两档，设计文档的 600/900
由浏览器按最近档渲染（题字层已由 Ma Shan Zheng 承担，影响可忽略）。"""

import pathlib
import re
import urllib.request

UA = (
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
    "(KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
)
OUT_DIR = pathlib.Path(__file__).resolve().parents[1] / "apps/web/public/fonts"
CSS_OUT = (
    pathlib.Path(__file__).resolve().parents[1]
    / "apps/web/app/styles/fonts-shanhaijing.css"
)

# 两族都只有 400 单字重（书体/仿宋本无多字重）
FAMILIES = [
    ("Ma+Shan+Zheng", "mashanzheng"),
    ("ZCOOL+XiaoWei", "zcoolxiaowei"),
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
        "/* 山海经字体自托管（由 scripts/shanhaijing-fonts.py 生成，勿手改）\n"
        "   来源：Google Fonts（Ma Shan Zheng / ZCOOL XiaoWei）\n"
        "   说明：保留原始 unicode-range 分片，浏览器按需加载；文件在 public/fonts/。\n"
        "   Noto Serif SC 复用 fonts-tide.css 已注册族（400/700），此处不重复。\n"
        "   更新方式：改 shanhaijing-fonts.py 的 FAMILIES 后重跑。 */\n\n"
    )
    CSS_OUT.write_text(header + "\n".join(blocks), encoding="utf-8")
    mb = total_bytes / 1024 / 1024
    print(f"\n完成：{total_files} 个文件，本轮新增下载 {mb:.2f} MB", flush=True)
    print(f"CSS 写入 {CSS_OUT}", flush=True)


if __name__ == "__main__":
    main()

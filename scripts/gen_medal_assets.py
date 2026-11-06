"""生成站内勋章图（2026-11-06 迁站补图）。

背景：旧站 NexusPHP 的 `medals.image_large/image_small` 存的是**外链图床
URL**（img.ptang.top 79 枚 / img.seedvault.cn 2 枚 / wimg.588ku.com 1 枚）。
迁站时只搬了勋章目录、没搬这两个字段 ⇒ 新站 `medals.asset_ref` 全 NULL，
前台一律回落「首字徽章」，用户看到的就不是旧站那批图。
更糟的是：img.ptang.top **域名已失效**（DNS 无解析），79 枚原图取不回。

本脚本产出（写进 apps/web/public/medals/，由 web 镜像静态托管）：
  1. 还能下到的原图（3 枚）→ 直接落站内，文件名 medal_<id>.<ext>；
  2. 取不到原图的（79 枚）→ 按 id 生成的岩彩风 SVG 徽章，文件名 medal_<id>.svg；
  3. 同时产出回填 SQL（asset_ref 指向站内路径）——外链随时可换回，
     只要改 asset_ref 即可，不必再动前端。

用法：
    python scripts/gen_medal_assets.py \
        --old-tsv _shots/medals_old.tsv \
        --rarity-tsv _shots/medal_rarity.tsv \
        --assets-dir _shots/medal_assets \
        --out-dir apps/web/public/medals \
        --sql-out _shots/medal_asset_ref.sql
"""

from __future__ import annotations

import argparse
import hashlib
import os
import shutil

# 稀有度 → (主色, 副色)：与 medal_rarities 的 mint/sky/indigo/gold 同族
RARITY_TONES = {
    "common": ("#6fbf9f", "#3fae8f"),
    "rare": ("#3fae8f", "#2f8f74"),
    "epic": ("#5cb3c7", "#3f7f92"),
    "legendary": ("#d3a13f", "#a97c22"),
    "gold": ("#d3a13f", "#a97c22"),
    "silver": ("#c9d2d6", "#9aa7ad"),
    "bronze": ("#c98a52", "#9a6337"),
}
DEFAULT_TONE = RARITY_TONES["common"]

FONT_STACK = (
    "'Noto Serif SC','Source Han Serif SC','Songti SC',"
    "'STSong','SimSun',serif"
)


def esc(text: str) -> str:
    """XML 文本转义。"""
    return (
        text.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
    )


def read_tsv(path: str) -> list[list[str]]:
    rows: list[list[str]] = []
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            line = line.rstrip("\n")
            if not line:
                continue
            rows.append(line.split("\t"))
    return rows


def pattern_svg(idx: int, main: str, sub: str) -> str:
    """按 id 选四种岩彩纹样之一（山形/波纹/点阵/金线）。"""
    kind = idx % 4
    parts: list[str] = []
    if kind == 0:
        parts.append(
            f'<path d="M20 100 L44 62 L64 88 L86 54 L108 100 Z" '
            f'fill="{sub}" opacity="0.55"/>'
        )
        parts.append(
            f'<path d="M26 100 L48 74 L66 96 L88 68 L104 100 Z" '
            f'fill="{main}" opacity="0.35"/>'
        )
    elif kind == 1:
        for k in range(3):
            r = 22 + k * 14
            parts.append(
                f'<circle cx="64" cy="86" r="{r}" fill="none" '
                f'stroke="{sub}" stroke-width="2" opacity="0.45"/>'
            )
    elif kind == 2:
        for row in range(4):
            for col in range(4):
                cx = 34 + col * 20 + (10 if row % 2 else 0)
                cy = 38 + row * 18
                parts.append(
                    f'<circle cx="{cx}" cy="{cy}" r="3.2" fill="{sub}" '
                    f'opacity="0.5"/>'
                )
    else:
        for k in range(4):
            off = k * 18
            parts.append(
                f'<path d="M18 {44 + off} L110 {28 + off}" stroke="{sub}" '
                f'stroke-width="3" opacity="0.4"/>'
            )
    return "".join(parts)


def build_svg(mid: int, name: str, rarity: str) -> str:
    """生成一枚 128×128 的岩彩风 SVG 徽章。"""
    main, sub = RARITY_TONES.get(rarity, DEFAULT_TONE)
    seed = int(hashlib.sha256(f"{mid}:{name}".encode()).hexdigest()[:8], 16)
    angle = 20 + seed % 120
    initial = (name.strip()[:1] or "章").upper()
    gid = f"g{mid}"
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" '
        f'viewBox="0 0 128 128" role="img" aria-label="{esc(name)}">'
        f"<defs>"
        f'<linearGradient id="{gid}a" '
        f'gradientTransform="rotate({angle} .5 .5)">'
        f'<stop offset="0%" stop-color="{main}"/>'
        f'<stop offset="100%" stop-color="{sub}"/>'
        f"</linearGradient>"
        f'<radialGradient id="{gid}b" cx="32%" cy="26%" r="72%">'
        f'<stop offset="0%" stop-color="#ffffff" stop-opacity="0.42"/>'
        f'<stop offset="60%" stop-color="#ffffff" stop-opacity="0.06"/>'
        f'<stop offset="100%" stop-color="#000000" stop-opacity="0.22"/>'
        f"</radialGradient>"
        f"</defs>"
        f'<circle cx="64" cy="64" r="61" fill="url(#{gid}a)"/>'
        f"{pattern_svg(mid, main, sub)}"
        f'<circle cx="64" cy="64" r="61" fill="url(#{gid}b)"/>'
        f'<circle cx="64" cy="64" r="57" fill="none" stroke="#f0d795" '
        f'stroke-width="2" opacity="0.75"/>'
        f'<circle cx="64" cy="64" r="52" fill="none" stroke="#1c1408" '
        f'stroke-width="1" opacity="0.28"/>'
        f'<text x="64" y="67" text-anchor="middle" '
        f'dominant-baseline="central" font-family="{FONT_STACK}" '
        f'font-size="54" font-weight="700" fill="#fff6e2" '
        f'stroke="#2a1f12" stroke-width="2.4" paint-order="stroke" '
        f'stroke-linejoin="round">{esc(initial)}</text>'
        f"</svg>"
    )


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--old-tsv", default="_shots/medals_old.tsv")
    ap.add_argument("--rarity-tsv", default="_shots/medal_rarity.tsv")
    ap.add_argument("--assets-dir", default="_shots/medal_assets")
    ap.add_argument("--out-dir", default="apps/web/public/medals")
    ap.add_argument("--sql-out", default="_shots/medal_asset_ref.sql")
    args = ap.parse_args()

    olds = read_tsv(args.old_tsv)
    rarities = {
        r[0]: (r[1] if len(r) > 1 else "common")
        for r in read_tsv(args.rarity_tsv)
    }
    os.makedirs(args.out_dir, exist_ok=True)
    sql_lines = [
        "-- 勋章图回填（站内静态托管；旧站外链图床 79/82 已失效）",
        "BEGIN;",
    ]
    n_orig = 0
    n_svg = 0
    for row in olds:
        if len(row) < 2:
            continue
        mid = row[0].strip()
        name = row[1].strip()
        rarity = rarities.get(mid, "common").strip() or "common"
        found = None
        if os.path.isdir(args.assets_dir):
            for fn in sorted(os.listdir(args.assets_dir)):
                if fn.startswith(f"old_{mid}."):
                    found = fn
                    break
        if found:
            ext = found.rsplit(".", 1)[-1]
            fname = f"medal_{mid}.{ext}"
            shutil.copyfile(
                os.path.join(args.assets_dir, found),
                os.path.join(args.out_dir, fname),
            )
            n_orig += 1
        else:
            fname = f"medal_{mid}.svg"
            with open(
                os.path.join(args.out_dir, fname),
                "w",
                encoding="utf-8",
                newline="\n",
            ) as fh:
                fh.write(build_svg(int(mid), name, rarity))
            n_svg += 1
        sql_lines.append(
            f"UPDATE medals SET asset_ref='/medals/{fname}' WHERE id={mid};"
        )
    sql_lines.append("COMMIT;")
    with open(args.sql_out, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(sql_lines) + "\n")
    print(f"originals copied: {n_orig}, svg generated: {n_svg}")
    print(f"out dir: {args.out_dir}")
    print(f"sql: {args.sql_out}")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""构建 auto_feed 的 FluxTorrent 适配版。

把 core.js + adapter.js 注入上游 auto_feed.user.js，并把 @require 依赖
换成境内可达镜像（上游默认指向 greasyfork/unpkg，国内网络常加载失败）。

用法：
    python scripts/userscripts/auto-feed-flux/build.py
    python scripts/userscripts/auto-feed-flux/build.py --src _tmp/auto_feed.user.js
    python scripts/userscripts/auto-feed-flux/build.py --check

产物：dist/auto-feed-flux.user.js（dist/ 已被 .gitignore 忽略）
"""

from __future__ import annotations

import argparse
import hashlib
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
DEFAULT_OUT = HERE / "dist" / "auto-feed-flux.user.js"

UPSTREAM_URLS = [
    "https://raw.githubusercontent.com/tomorrow505/auto_feed_js/"
    "master/auto_feed.user.js",
    "https://ghproxy.net/https://raw.githubusercontent.com/"
    "tomorrow505/auto_feed_js/master/auto_feed.user.js",
    "https://gitee.com/tomorrow505/auto_feed_js/raw/master/"
    "auto_feed.user.js",
]

LIVE_SITE = "https://fluxtorrent.ptang.top/"
LIVE_ICON = LIVE_SITE + "icons/icon-192.png"
# namespace 是 TM 的脚本更新身份，不随域名走；改了会被当成新脚本。
NAMESPACE = "https://fluxtorrent.ptang.top/auto-feed"

# 上游 -> 镜像（@require 行逐条替换；命中数必须为 1）
REQUIRE_MIRRORS = [
    (
        "https://unpkg.com/@popperjs/core@2/dist/umd/popper.min.js",
        "https://cdn.jsdelivr.net/npm/@popperjs/core@2/dist/umd/"
        "popper.min.js",
    ),
    (
        "https://unpkg.com/tippy.js@6/dist/tippy-bundle.umd.js",
        "https://cdn.jsdelivr.net/npm/tippy.js@6/dist/"
        "tippy-bundle.umd.min.js",
    ),
    (
        "https://greasyfork.org/scripts/453166-jquery/code/jquery.js"
        "?version=1105525",
        "https://cdn.jsdelivr.net/npm/jquery@3.7.1/dist/jquery.min.js",
    ),
    (
        "https://greasyfork.org/scripts/28502-jquery-ui-v1-11-4/code/"
        "jQuery%20UI%20-%20v1114.js?version=187735",
        "https://cdnjs.cloudflare.com/ajax/libs/jqueryui/1.11.4/"
        "jquery-ui.min.js",
    ),
    (
        "https://greasyfork.org/scripts/430180-imgcheckbox2/code/"
        "imgCheckbox2.js?version=956211",
        "https://gitee.com/tomorrow505/auto-feed-helper/raw/master/"
        "require_libraries/imgCheckbox.js",
    ),
    (
        "https://greasyfork.org/scripts/444988-music-helper/code/"
        "music-helper.js?version=1268106",
        "https://gitee.com/tomorrow505/auto-feed-helper/raw/master/"
        "require_libraries/music-helper.js",
    ),
]

MATCH_LINES = """\
// @match        *://fluxtorrent.ptang.top/*
// @match        *://ft.ptang.top/*
// @match        *://mfwg.ptang.top/*
// @match        *://localhost:3000/*
// @match        *://127.0.0.1:3000/*
// @match        *://*/torrent/*
// @match        *://*/torrents*
// @match        *://*/upload/*
// @match        *://*/upload
"""


def fail(msg: str) -> None:
    print("ERROR: " + msg, file=sys.stderr)
    raise SystemExit(1)


def replace_once(src: str, old: str, new: str, name: str) -> str:
    n = src.count(old)
    if n != 1:
        fail(f"锚点「{name}」命中 {n} 次（应为 1），上游可能已改动")
    return src.replace(old, new, 1)


def fetch_upstream(src_arg: str | None) -> str:
    if src_arg:
        path = Path(src_arg)
        if not path.is_file():
            fail(f"--src 文件不存在：{path}")
        return path.read_text(encoding="utf-8")
    errors = []
    for url in UPSTREAM_URLS:
        try:
            print(f"下载上游：{url}")
            req = urllib.request.Request(
                url, headers={"User-Agent": "Mozilla/5.0 flux-auto-feed"}
            )
            with urllib.request.urlopen(req, timeout=90) as resp:
                data = resp.read()
            # 上游实测 CRLF/LF 不定（镜像链会改写行尾）；锚点全部按 LF 写，
            # 拉下来先归一，否则跨行锚在 CRLF 版必挂（2026-10-31 实测）
            text = data.decode("utf-8").replace("\r\n", "\n")
            if len(data) < 1_000_000:
                errors.append(f"{url}: 体积异常 {len(data)}")
                continue
            if "function auto_feed() {" not in text:
                errors.append(f"{url}: 不含 auto_feed 入口")
                continue
            return text
        except Exception as exc:  # noqa: BLE001 - 逐个镜像尝试
            errors.append(f"{url}: {exc}")
    fail("上游下载失败：\n  " + "\n  ".join(errors))


def patch_header(src: str) -> str:
    src = replace_once(
        src,
        "// @name         auto_feed",
        "// @name         auto_feed (FluxTorrent)",
        "header name",
    )
    src = replace_once(
        src,
        "// @namespace    https://greasyfork.org/zh-CN/scripts/424132-auto-feed",
        "// @namespace    " + NAMESPACE,
        "header namespace",
    )
    src = replace_once(
        src,
        "// @description  PT一键转种脚本",
        "// @description  PT一键转种脚本（FluxTorrent 适配版）",
        "header description",
    )
    src = replace_once(
        src,
        "// @updateURL    https://greasyfork.org/zh-CN/scripts/"
        "424132-auto-feed\n",
        "",
        "header updateURL",
    )
    src = replace_once(
        src,
        "// @version      2.1.1.4",
        "// @version      2.1.1.4.6",
        "header version",
    )
    src = replace_once(
        src,
        "// @icon         https://kp.m-team.cc//favicon.ico",
        "// @icon         " + LIVE_ICON,
        "header icon",
    )
    src = replace_once(
        src,
        "// @run-at       document-end\n",
        "// @run-at       document-end\n" + MATCH_LINES,
        "header match",
    )
    for old, new in REQUIRE_MIRRORS:
        src = replace_once(src, old, new, "@require " + old[:48])
    return src


def inject_adapter(src: str) -> str:
    core = (HERE / "core.js").read_text(encoding="utf-8")
    adapter = (HERE / "adapter.js").read_text(encoding="utf-8")
    block = (
        "\n/* ===== FluxTorrent adapter begin ===== */\n"
        + core.rstrip()
        + "\n\n"
        + adapter.rstrip()
        + "\n/* ===== FluxTorrent adapter end ===== */\n\n"
    )
    return replace_once(
        src,
        "var site_url = decodeSiteURL();\n",
        "var site_url = decodeSiteURL();\n" + block,
        "site_url anchor",
    )


def patch_host_prompt(src: str) -> str:
    old = (
        'if (GM_getValue("host_link") === undefined && '
        "judge_if_the_site_as_source(site_url) !== undefined) {"
    )
    new = (
        'if (GM_getValue("host_link") === undefined && '
        "judge_if_the_site_as_source(site_url) !== undefined && "
        "!fluxIsTorrentSite()) {"
    )
    return replace_once(src, old, new, "host_link prompt")


def patch_registry(src: str) -> str:
    anchor = "'好学': {'url': 'https://www.hxpt.org/', 'enable': 1},"
    line = f"    'FluxTorrent': {{'url': '{LIVE_SITE}', 'enable': 1}},"
    return replace_once(
        src, anchor, anchor + "\n" + line, "default_site_info",
    )


def patch_jump_href(src: str) -> str:
    anchor = (
        "                } else if (key == 'MTeam' || key == 'RouSi') {\n"
        "                    forward_url = used_site_info[key].url + "
        "'upload';\n"
        "                } else if (key == '影') {"
    )
    added = (
        "                } else if (key == 'MTeam' || key == 'RouSi') {\n"
        "                    forward_url = used_site_info[key].url + "
        "'upload';\n"
        "                } else if (key == 'FluxTorrent') {\n"
        "                    forward_url = used_site_info[key].url + "
        "'upload';\n"
        "                } else if (key == '影') {"
    )
    return replace_once(src, anchor, added, "set_jump_href")


def patch_fill_torrent(src: str) -> str:
    anchor = (
        "    } else if (forward_site == 'RouSi') {\n"
        '        var i_evt = new Event("change", { bubbles: true, '
        "cancelable: false });\n"
        "        $('input[accept=\".torrent\"]')[0].files = "
        "container.files;\n"
        "        $('input[accept=\".torrent\"]')[0].dispatchEvent(i_evt);\n"
        "    } else {\n"
        "        $('input[name=file]')[0].files = container.files;\n"
        "    }\n"
    )
    added = (
        "    } else if (forward_site == 'RouSi') {\n"
        '        var i_evt = new Event("change", { bubbles: true, '
        "cancelable: false });\n"
        "        $('input[accept=\".torrent\"]')[0].files = "
        "container.files;\n"
        "        $('input[accept=\".torrent\"]')[0].dispatchEvent(i_evt);\n"
        "    } else if (forward_site == 'FluxTorrent') {\n"
        "        var fluxInput = document.querySelector(\n"
        "            '#upload-form input[type=\"file\"][accept*=\".torrent\"]'\n"
        "        );\n"
        "        if (fluxInput) {\n"
        "            fluxInput.files = container.files;\n"
        "            fluxInput.dispatchEvent(new Event('change', "
        "{ bubbles: true }));\n"
        "        }\n"
        "    } else {\n"
        "        $('input[name=file]')[0].files = container.files;\n"
        "    }\n"
    )
    return replace_once(src, anchor, added, "fill_torrent")


def patch_add_torrent(src: str) -> str:
    anchor = (
        "function addTorrent(url, name, forward_site, forward_announce) {\n"
        "    url = url || '';\n"
        "    name = name || raw_info?.torrent_name || raw_info?.name || "
        "'auto_feed';\n"
    )
    added = anchor + """    if (raw_info && raw_info.torrent_b64_key &&
        typeof GM_getValue === "function") {
        try {
            var fluxStored = GM_getValue("flux_torrent_b64");
            if (fluxStored) {
                var fluxObj = JSON.parse(fluxStored);
                var fluxAge = Date.now() - (fluxObj.at || 0);
                if (fluxObj.key === raw_info.torrent_b64_key &&
                    fluxObj.b64 && fluxAge >= 0 &&
                    fluxAge < 30 * 60 * 1000) {
                    var fluxBin = atob(fluxObj.b64);
                    build_blob_from_torrent(fluxBin, forward_announce,
                        forward_site, "application/x-bittorrent",
                        function (data) {
                            var fluxBlob = data.data;
                            var fluxFile = new window.File([fluxBlob],
                                name + '.torrent', { type: fluxBlob.type });
                            var fluxDt = new DataTransfer();
                            fluxDt.items.add(fluxFile);
                            fill_torrent(forward_site, fluxDt, name);
                        });
                    return;
                }
            }
        } catch (fluxErr) {
            console.warn("flux torrent cache failed", fluxErr);
        }
    }
"""
    return replace_once(src, anchor, added, "addTorrent b64")


def patch_auto_feed(src: str) -> str:
    old = "function auto_feed() {\n"
    new = (
        "function auto_feed() {\n"
        "    if (fluxIsTorrentSite()) {\n"
        "        FluxAutoFeedAdapter.main();\n"
        "        return;\n"
        "    }\n"
    )
    return replace_once(src, old, new, "auto_feed entry")


def validate(src: str) -> None:
    need = [
        "FluxAutoFeedCore",
        "FluxAutoFeedAdapter",
        "fluxIsTorrentSite",
        "'FluxTorrent': {'url': '" + LIVE_SITE,
        "@match        *://*/upload",
        "fluxInput.dispatchEvent",
        "torrent_b64_key",
        "flux_torrent_b64",
        "!fluxIsTorrentSite()",
    ]
    for marker in need:
        if marker not in src:
            fail(f"产物缺少标记：{marker}")
    if "// @updateURL" in src:
        fail("产物仍带 @updateURL，会被上游覆盖")
    if re.search(r"@require\s+https://greasyfork\.org/", src):
        fail("产物仍引用 greasyfork @require")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--src", help="本地上游 auto_feed.user.js")
    ap.add_argument("--out", default=str(DEFAULT_OUT))
    ap.add_argument("--check", action="store_true", help="只校验不写文件")
    args = ap.parse_args()

    upstream = fetch_upstream(args.src)
    digest = hashlib.sha256(upstream.encode("utf-8")).hexdigest()
    print(f"上游 sha256={digest[:16]}… size={len(upstream)}")

    src = patch_header(upstream)
    src = inject_adapter(src)
    src = patch_host_prompt(src)
    src = patch_registry(src)
    src = patch_jump_href(src)
    src = patch_fill_torrent(src)
    src = patch_add_torrent(src)
    src = patch_auto_feed(src)
    validate(src)

    if args.check:
        print("check OK")
        return

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w", encoding="utf-8", newline="\n") as fh:
        fh.write(src)
    print(f"产物：{out}  ({out.stat().st_size} bytes)")

    try:
        subprocess.run(
            ["node", "--check", str(out)],
            check=True,
            capture_output=True,
            text=True,
        )
        print("node --check OK")
    except FileNotFoundError:
        print("未找到 node，跳过语法检查")
    except subprocess.CalledProcessError as exc:
        fail("node --check 失败：\n" + exc.stderr)


if __name__ == "__main__":
    main()

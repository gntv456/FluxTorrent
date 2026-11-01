# -*- coding: utf-8 -*-
"""GHCR 陈旧镜像清理：删四包 0.1.x–0.4.0 版本，保留 0.5.x / dev.* / latest。

红线（不删）：0.5.0 / 0.5.1 / 0.5.2-dev.*（现网与回滚窗口）/ latest /
各 *-amd64 *-arm64 架构 tag 随主 tag 同包版本删除（同 version id）。
用法：先 DRYRUN=1 看，再去掉跑。
"""
import os
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
DRY = os.environ.get("DRYRUN") == "1"
PKGS = ["api", "web", "worker", "tracker"]
KEEP_PREFIX = ("0.5.",)  # 0.5.0/0.5.1/0.5.2-* 全保留


def gh(*args):
    r = subprocess.run(["gh", "api", *args], capture_output=True,
                       text=True, encoding="utf-8")
    if r.returncode != 0:
        raise SystemExit(f"gh api 失败: {r.stderr[:200]}")
    return r.stdout


def pkg_versions(pkg):
    out = gh(f"user/packages/container/fluxtorrent%2F{pkg}/versions",
             "--paginate")
    import json
    return json.loads(out)  # list of {id, metadata.container.tags}


total_deleted = 0
for pkg in PKGS:
    vers = pkg_versions(pkg)
    for v in vers:
        tags = (v.get("metadata", {}).get("container", {}) or {}).get("tags")
        if not tags:
            # 无 tag 的版本层（被多架构 manifest 引用或 dangling）
            continue
        keep = any(t == "latest" or t.startswith(KEEP_PREFIX)
                   for t in tags)
        if keep:
            continue
        label = ",".join(tags)
        if DRY:
            print(f"[dry] {pkg} v{v['id']} tags={label} -> 将删除")
            continue
        gh(f"user/packages/container/fluxtorrent%2F{pkg}/versions/"
           f"{v['id']}", "-X", "DELETE")
        print(f"已删 {pkg} {label}")
        total_deleted += 1
print(f"{'DRYRUN ' if DRY else ''}完成，删除 {total_deleted} 个版本层")

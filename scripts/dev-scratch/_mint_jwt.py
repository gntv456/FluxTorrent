"""签一个 staff JWT（供 Playwright / curl 冒烟脚本用）。

用法：
    FLUX_TOKEN=$(python _mint_jwt.py) node apps/web/_chk_m45_pages.mjs
    python _mint_jwt.py --uid 1 --class 99

要点：密钥取自容器内 `JWT_SECRET` 环境变量，claims 与
`apps/api` 的 `require_auth` 同口径（sub/class_id/iat/exp，HS256）。
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import hmac
import json
import subprocess
import time


def b64url(raw: bytes) -> str:
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode()


def secret(container: str) -> str:
    out = subprocess.run(
        ["docker", "exec", container, "printenv", "JWT_SECRET"],
        capture_output=True,
        text=True,
        check=True,
    )
    return out.stdout.strip()


def token(uid: int, cls: int, ttl: int, container: str) -> str:
    now = int(time.time())
    head = b64url(json.dumps({"alg": "HS256", "typ": "JWT"}).encode())
    body = b64url(
        json.dumps(
            {"sub": uid, "class_id": cls, "iat": now, "exp": now + ttl}
        ).encode()
    )
    sig = hmac.new(
        secret(container).encode(), f"{head}.{body}".encode(),
        hashlib.sha256,
    ).digest()
    return f"{head}.{body}.{b64url(sig)}"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--uid", type=int, default=1, help="用户 id（默认站长 1）")
    ap.add_argument("--class", type=int, default=99, dest="cls",
                    help="用户 class_id（99=站长）")
    ap.add_argument("--ttl", type=int, default=3600, help="有效期秒")
    ap.add_argument("--container", default="flux-api", help="api 容器名")
    a = ap.parse_args()
    print(token(a.uid, a.cls, a.ttl, a.container))


if __name__ == "__main__":
    main()

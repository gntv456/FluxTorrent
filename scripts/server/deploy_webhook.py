#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""FluxTorrent 部署 webhook 接收器（2026-11-01 发版自动化）。

GitHub release-automation 在发版完成后 POST 本服务：
  {"version": "0.5.3", "ref": "v0.5.3"}
头部 X-Hub-Signature-256: sha256=HMAC_SHA256(body, DEPLOY_WEBHOOK_SECRET)。

动作：校验签名 → .env 的 FLUX_VERSION 改为目标版 → compose pull + up -d。
安全面：仅 HMAC 校验通过才动手；监听 127.0.0.1（外网经反代/SSH 隧道到达，
反代层再限源 IP）。单线程 http.server 足够（发版频率一天数次）。

部署（服务器上）：
  nohup python3 deploy_webhook.py >> /var/log/flux-webhook.log 2>&1 &
  # 或写进 systemd unit；密钥放 /etc/flux-webhook.secret（0600）
"""
import hmac
import json
import os
import subprocess
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

for s in (sys.stdout, sys.stderr):
    s.reconfigure(encoding="utf-8", errors="replace")

COMPOSE_DIR = os.environ.get("FLUX_COMPOSE_DIR",
                             os.path.expanduser("~/FluxTorrent/docker"))
SECRET_FILE = os.environ.get("FLUX_WEBHOOK_SECRET",
                             "/etc/flux-webhook.secret")
LISTEN = ("127.0.0.1", int(os.environ.get("FLUX_WEBHOOK_PORT", "8741")))


def log(msg: str) -> None:
    print(f"[{__import__('datetime').datetime.now():%F %T}] {msg}",
          flush=True)


def deploy(version: str) -> tuple[bool, str]:
    env_path = os.path.join(COMPOSE_DIR, ".env")
    src = open(env_path, encoding="utf-8").read()
    lines, hit = [], False
    for ln in src.splitlines():
        if ln.startswith("FLUX_VERSION="):
            lines.append(f"FLUX_VERSION={version}")
            hit = True
        else:
            lines.append(ln)
    if not hit:
        return False, ".env 无 FLUX_VERSION 行"
    open(env_path, "w", encoding="utf-8", newline="\n").write(
        "\n".join(lines) + "\n")
    r = subprocess.run(
        ["docker", "compose", "-f", "docker-compose.yml", "pull",
         "api", "web", "worker", "tracker"],
        cwd=COMPOSE_DIR, capture_output=True, text=True, timeout=1800)
    if r.returncode != 0:
        return False, f"pull 失败: {r.stderr[-400:]}"
    r = subprocess.run(
        ["docker", "compose", "-f", "docker-compose.yml", "up", "-d"],
        cwd=COMPOSE_DIR, capture_output=True, text=True, timeout=600)
    if r.returncode != 0:
        return False, f"up 失败: {r.stderr[-400:]}"
    return True, f"已更新到 {version}"


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):  # noqa: N802（http.server 命名约定）
        if self.path != "/deploy":
            self.send_error(404)
            return
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length)
        secret = open(SECRET_FILE, encoding="utf-8").read().strip().encode()
        expect = "sha256=" + hmac.new(secret, body, "sha256").hexdigest()
        got = self.headers.get("X-Hub-Signature-256", "")
        if not hmac.compare_digest(expect, got):
            log(f"签名校验失败 path={self.path}")
            self.send_error(403)
            return
        try:
            version = json.loads(body)["version"]
        except Exception:
            self.send_error(400)
            return
        log(f"收到部署请求 version={version}")
        ok, msg = deploy(version)
        log(f"结果 ok={ok} {msg}")
        self.send_response(200 if ok else 500)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps({"ok": ok, "msg": msg}).encode())

    def log_message(self, fmt, *args):  # 静默默认访问日志，走自己的 log()
        pass


if __name__ == "__main__":
    log(f"webhook 监听 {LISTEN}，compose 目录 {COMPOSE_DIR}")
    HTTPServer(LISTEN, Handler).serve_forever()

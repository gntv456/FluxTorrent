#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""FluxTorrent 部署 webhook 接收器（2026-11-01 发版自动化）。

GitHub release-automation 在发版完成后 POST 本服务：
  {"version": "0.5.3", "ref": "v0.5.3"}
头部 X-Hub-Signature-256: sha256=HMAC_SHA256(body, DEPLOY_WEBHOOK_SECRET)。

动作：校验签名 → 把目标版本钉进 .env → 交 scripts/update.sh 执行升级。
**升级本体不在这里重做**：迁移守卫、健康检查、失败回滚只在 update.sh 里有
一份。早先这里自己写了「pull + up -d」一条链，等于给同一件事开第二个口径
——而 2026-11-02 实测过一次真事故形状：库里已应用到 0450，已发布的 0.5.3
镜像只到 0449，直接 `up -d` 会让 api 崩溃循环、全站 500，且这条链既无守卫
也不看健康，还会向 GitHub 回 ok。

安全面：仅 HMAC 校验通过才动手；version 先过形状校验再进 .env（.env 是
compose 的注入面，多行/奇怪字符的「版本号」会把键值对写进站点配置）。
监听 127.0.0.1（外网经反代/SSH 隧道到达，反代层再限源 IP）。

部署（服务器上）：
  nohup python3 deploy_webhook.py >> /var/log/flux-webhook.log 2>&1 &
  # 或写进 systemd unit；密钥放 /etc/flux-webhook.secret（0600）
"""
import hmac
import json
import os
import re
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

# 版本号形状：必须以数字开头，只含半角字母/数字/点/横杠/下划线，且不含
# 换行——`.env` 一行一键，带换行的「版本」等于往站点配置里塞别的键。
VERSION_RE = re.compile(r"^[0-9][0-9A-Za-z._-]{0,63}$")


def log(msg: str) -> None:
    print(f"[{__import__('datetime').datetime.now():%F %T}] {msg}",
          flush=True)


def pin_version(env_path: str, version: str) -> str:
    """把 FLUX_VERSION 钉进 .env，返回**改前原文**（失败时原样还原）。

    写盘走 temp + os.replace：直接 `open(path,'w')` 是截断后写，进程在中途
    被杀就留下一份残缺 .env——那里面是全站口令与密钥。
    """
    src = open(env_path, encoding="utf-8").read()
    lines = src.splitlines()
    out, hit = [], False
    for ln in lines:
        if ln.startswith("FLUX_VERSION="):
            out.append(f"FLUX_VERSION={version}")
            hit = True
        else:
            out.append(ln)
    if not hit:
        # 默认安装只有 .env.example 里那行注释掉的 `#FLUX_VERSION=latest`，
        # 不补就会让自动发版永远停在「.env 无 FLUX_VERSION 行」
        out.append(f"FLUX_VERSION={version}")
    tmp = env_path + ".tmp"
    with open(tmp, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(out) + "\n")
    os.replace(tmp, env_path)
    return src


def deploy(version: str):
    """执行升级，返回 (是否成功, 给 GitHub/日志看的说明)。

    返回值刻意不写成 `tuple[bool, str]`：那种下标注解要 Python ≥3.9，而这台
    服务器上跑的是 3.8 —— 老运行时会连 import 都过不去（TypeError 直接炸），
    表现为「webhook 根本起不来」。
    """
    env_path = os.path.join(COMPOSE_DIR, ".env")
    if not os.path.isfile(env_path):
        return False, f"找不到 {env_path}"
    root = os.path.dirname(os.path.abspath(COMPOSE_DIR))
    updater = os.path.join(root, "scripts", "update.sh")
    if not os.path.isfile(updater):
        return False, (f"找不到 {updater}：升级必须走 update.sh 的迁移守卫"
                       "与健康检查，这里不代替它 pull/up")
    src = pin_version(env_path, version)
    r = subprocess.run(
        ["bash", updater, "--tag", version, "--yes"],
        cwd=root, capture_output=True, text=True,
        encoding="utf-8", errors="replace", timeout=2400)
    tail = ((r.stdout or "")[-1500:] + "\n" + (r.stderr or "")[-400:]).strip()
    if r.returncode != 0:
        # update.sh 失败时会把容器回滚到上一版本，但 .env 的钉版是刚改的——
        # 不回写就等于留下一份「下次 compose up 才炸」的定时炸弹
        tmp = env_path + ".tmp"
        with open(tmp, "w", encoding="utf-8", newline="\n") as f:
            f.write(src if src.endswith("\n") else src + "\n")
        os.replace(tmp, env_path)
        return False, f"update.sh 退出码 {r.returncode}，.env 已还原：\n{tail}"
    return True, f"已更新到 {version}\n{tail[-500:]}"


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
        if not isinstance(version, str) or not VERSION_RE.match(version):
            # 形状不对就别碰 .env，也别拉镜像。状态行 reason 只许 ASCII：
            # BaseHTTPRequestHandler 把它写进 HTTP 状态行并按 latin-1 编码，
            # 中文短语会在这里抛 UnicodeEncodeError，客户端只看到
            # 「连接被关掉、一个字节都没回」（2026-11-02 实测）。
            log(f"拒绝非法 version：{version!r}")
            self.send_error(400, "bad version (expected like 0.5.3)")
            return
        log(f"收到部署请求 version={version}")
        ok, msg = deploy(version)
        log(f"结果 ok={ok} {msg[:200]}")
        self.send_response(200 if ok else 500)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps({"ok": ok, "msg": msg}).encode())

    def log_message(self, fmt, *args):  # 静默默认访问日志，走自己的 log()
        pass


if __name__ == "__main__":
    log(f"webhook 监听 {LISTEN}，compose 目录 {COMPOSE_DIR}")
    HTTPServer(LISTEN, Handler).serve_forever()

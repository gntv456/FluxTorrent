"""管理员代改密码（邮件通道不可用时的登录兜底）。

背景（2026-11-06）：新服务器机房封禁邮件出站（25/465/587 全超时，
443 通），用户申请「忘记密码」后收不到重置邮件 → 表现为登录不了。
本脚本让站长在服务器上直接给指定用户重置密码并拿到临时口令，
口令经 QQ/微信人工转交即可登录；用户下次登录后透明升级 argon2id。

用法（服务器侧）：
    python3 scripts/admin_reset_password.py <username|uid> [选项]

选项：
    --password PW   指定临时口令（默认随机 12 位，含大小写+数字）
    --totp-clear    同时清除 TOTP（旧站迁过来的 2FA，换设备后算不出码）
    --force-reset   置 must_reset_password=true，用户登录后必须改密
    --revoke        撤销该用户已签发的会话 token（改密后旧链接失效）
    --dry-run       只查用户并打印，不落库

依赖：服务器 python3 需有 bcrypt（pip install bcrypt）；无 bcrypt 时
脚本会用 openssl/htpasswd 均不可用 → 直接报错提示，不做明文降级。
"""

from __future__ import annotations

import argparse
import secrets
import string
import subprocess
import sys

PG_CONTAINER = "flux-postgres"
DB_USER = "flux"
DB_NAME = "fluxtorrent"

Bcrypt = None


def psql(sql: str) -> str:
    """在 PG 容器里跑 SQL，返回 stdout（去空行）。"""
    proc = subprocess.run(
        [
            "docker", "exec", "-i", PG_CONTAINER,
            "psql", "-U", DB_USER, "-d", DB_NAME, "-Atc", sql,
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or "psql failed")
    return proc.stdout.strip()


def gen_password(n: int = 12) -> str:
    """随机口令：去掉易混字符（0/O/1/l/I）。"""
    pool = string.ascii_letters + string.digits
    bad = set("0O1lI")
    while True:
        pw = "".join(secrets.choice(pool) for _ in range(n))
        if all(c not in bad for c in pw):
            return pw


def bcrypt_hash(password: str) -> str:
    """生成 bcrypt 哈希（$2b$）。FT 登录走 legacy bcrypt 分支后自动升级 argon2。"""
    global Bcrypt
    if Bcrypt is None:
        try:
            import bcrypt  # noqa: N813
        except ImportError:
            sys.exit(
                "缺少 bcrypt 模块：pip install bcrypt"
                "（不要用明文/自造格式降级，FT 只认 argon2/bcrypt/NP-sha256）"
            )
        Bcrypt = bcrypt
    return Bcrypt.hashpw(
        password.encode("utf-8"), Bcrypt.gensalt(rounds=10)
    ).decode("ascii")


def lookup(key: str) -> dict | None:
    """按用户名或数字 uid 查用户（只取必要列）。"""
    if key.isdigit():
        sql = (
            "SELECT id, username, class_id, status, totp_enabled, "
            "must_reset_password FROM users WHERE id = " + key
        )
    else:
        safe = key.replace("'", "''")
        sql = (
            "SELECT id, username, class_id, status, totp_enabled, "
            "must_reset_password FROM users WHERE username = '" + safe + "'"
        )
    out = psql(sql)
    if not out:
        return None
    parts = out.split("|")
    return {
        "id": int(parts[0]),
        "username": parts[1],
        "class_id": int(parts[2]),
        "status": int(parts[3]),
        "totp_enabled": parts[4] == "t",
        "must_reset_password": parts[5] == "t",
    }


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("user", help="用户名或 uid")
    ap.add_argument("--password")
    ap.add_argument("--totp-clear", action="store_true")
    ap.add_argument("--force-reset", action="store_true")
    ap.add_argument("--revoke", action="store_true")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    user = lookup(args.user)
    if not user:
        sys.exit(f"✗ 找不到用户：{args.user}")
    print(
        f"用户 #{user['id']} {user['username']} "
        f"class={user['class_id']} status={user['status']} "
        f"totp={'on' if user['totp_enabled'] else 'off'}"
    )
    if args.dry_run:
        return

    password = args.password or gen_password()
    pw_hash = bcrypt_hash(password)

    sets = [
        "pass_hash = '" + pw_hash + "'",
        "legacy_secret = NULL",
        "must_reset_password = "
        + ("TRUE" if args.force_reset else "FALSE"),
    ]
    if args.totp_clear:
        sets.append("totp_enabled = FALSE")
        sets.append("totp_secret = NULL")
    sql = (
        "UPDATE users SET " + ", ".join(sets) + " WHERE id = " + str(user["id"])
    )
    psql(sql)

    if args.revoke:
        # 撤销旧会话：token_revocations 的 nbf 取当前时间戳，
        # 早于它的 JWT 一律拒绝（登出同一套机制）。user_id 是主键，
        # 必须 UPSERT —— DO NOTHING 会让「已撤销过」的用户拿不到新 nbf。
        psql(
            "INSERT INTO token_revocations (user_id, nbf) VALUES ("
            + str(user["id"]) + ", EXTRACT(EPOCH FROM now())::bigint) "
            "ON CONFLICT (user_id) DO UPDATE SET nbf = GREATEST("
            "token_revocations.nbf, EXCLUDED.nbf)"
        )

    print(f"✓ 已重置 #{user['id']} {user['username']} 的密码")
    if args.totp_clear:
        print("  · 已清除 TOTP")
    if args.force_reset:
        print("  · must_reset_password=true（下次登录须改密）")
    if args.revoke:
        print("  · 已撤销旧会话 token")
    print(f"临时口令：{password}")
    print("（用户首次登录后系统会自动把哈希升级为 argon2id）")


if __name__ == "__main__":
    main()

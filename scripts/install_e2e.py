"""空库装站闸门：从生产态首启一路验到「装完即可用」。

为什么要有这个文件：2026-09-25 的四审复验里，装站链路一次撞出四处致命断链
（迁移 0134 注释引用尚未创建的函数 / 装机白名单不含改密端点导致闸门互锁 /
purge_demo_data 删除顺序死结 / 自助改密不失效状态缓存），**全部只在干净卷上才现形**，
而 master 的 CI 自 09-18 之后就没成功过。所以这条链必须有个不依赖 CI 也能跑的入口：
    docker compose down -v && docker compose up -d && python scripts/install_e2e.py
CI 侧同样挂了这一步（.github/workflows/e2e-smoke.yml「空库装站闸门」）。

前提：本地 compose 栈（:8080）以生产态启动、数据库为空卷（未装机）。
脚本会随机改掉 root 口令并打印，只在测试实例上跑；已装机的库会直接拒绝退出。
"""
import json
import os
import secrets
import subprocess
import sys
import time
import urllib.error
import urllib.request

API = os.environ.get("FLUX_E2E_BASE", "http://127.0.0.1:8080/api/v1")
NEW_PW = "Flux#Inst" + secrets.token_urlsafe(9)
fails = []


def check(name, ok, detail=""):
    print(("PASS " if ok else "FAIL ") + name
          + ("  " + str(detail)[:150] if detail else ""))
    if not ok:
        fails.append(name)


def call(method, path, body=None, tok=None):
    req = urllib.request.Request(
        API + path, method=method,
        data=json.dumps(body).encode() if body is not None else None,
        headers={"Content-Type": "application/json",
                 **({"Authorization": "Bearer " + tok} if tok else {})})
    try:
        with urllib.request.urlopen(req, timeout=40) as r:
            return r.status, r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")


def code_of(body):
    try:
        return json.loads(body).get("code")
    except Exception:  # noqa
        return None


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode:
        raise SystemExit("psql 失败：" + r.stderr.strip()[:200])
    return r.stdout.strip()


# 0) 等 api 起来（迁移链在启动里跑，空库要 1~2 分钟）
for _ in range(90):
    if call("GET", "/health")[0] == 200:
        break
    time.sleep(2)
else:
    raise SystemExit("api /health 180s 未就绪：多半是迁移或启动期崩了，"
                     "先 docker compose logs api --tail 50")

# 1) 只接受「未装机」的库——避免在已装好的实例上误改口令
status = json.loads(call("GET", "/setup/status")[1]).get("data") or {}
if status.get("done"):
    raise SystemExit("该库已完成装机（setup_done=done），本闸门只验空库首启。"
                     "要重跑请在 docker/ 下执行：docker compose down -v "
                     "&& docker compose up -d")
check("S1 未装机且已有引导 root", status.get("has_admin") is True, status)

# 2) 生产态首启可登录（演示口令防线不得连带随机化 0017 root）
st, bd = call("POST", "/auth/login", {"username": "root",
                                      "password": "password123"})
tok = (json.loads(bd).get("data") or {}).get("token")
check("S2 root/password123 首启可登录", bool(tok), "HTTP%s" % st)
if not tok:
    raise SystemExit("首启即锁死，后续无从验证：%s" % bd[:200])

# 3) 未装完时业务 API 必须被封（防裸奔站被扫），但白名单要允许自救改密
check("S3 装机门封业务 API",
      code_of(call("GET", "/torrents", None, tok)[1]) != 0, "")
st, bd = call("POST", "/me/password/change", {"old_password": "password123",
                                              "new_password": NEW_PW}, tok)
check("S4 白名单内含改密自救通道", code_of(bd) == 0, bd[:150])

# 4) 改密后立即重登并完成任务（时序确定性：状态缓存必须已失效）
tok = (json.loads(call("POST", "/auth/login", {"username": "root",
                                               "password": NEW_PW})[1])
       .get("data") or {}).get("token")
check("S5 新口令可登录", bool(tok), "")
st, bd = call("POST", "/setup", {"pack": "general",
                                 "site_name": "闸门装机站",
                                 "games_compliance_ack": True}, tok)
check("S6 改密后紧接着完成向导（无 5s 假死）", code_of(bd) == 0, bd[:200])
check("S7 setup_done 已置位",
      (json.loads(call("GET", "/setup/status")[1]).get("data") or {}).get("done")
      is True, "")

# 5) 装完是真空站：演示数据清 0、词表来自包、业务 API 可用
check("S8 演示账号已清",
      psql("SELECT count(*) FROM users WHERE email LIKE '%@demo.local'") == "0", "")
check("S9 演示种子已清", psql("SELECT count(*) FROM torrents") == "0", "")
cats = int(psql("SELECT count(*) FROM categories"))
kinds = int(psql("SELECT count(*) FROM section_kinds"))
check("S10 general 包词表已铺", cats >= 7 and kinds >= 4,
      "分类 %d / 维度 %d" % (cats, kinds))
check("S11 装完可调业务 API",
      code_of(call("GET", "/torrents", None, tok)[1]) == 0, "")

# 6) 0214 装站守卫：首码自动发放 / announce 拒绝回环 / SMTP 测试端点存在
st, bd = call("POST", "/setup", {"pack": "", "site_name": "",
                                 "games_compliance_ack": True}, tok)
first_invite = (json.loads(bd).get("data") or {}).get("first_invite")
check("S12 邀请制自动产首码", isinstance(first_invite, str)
      and len(first_invite) >= 20,
      str(first_invite)[:60])
st, bd = call("POST", "/setup", {"pack": "", "site_name": "",
                                 "announce_url": "http://127.0.0.1:8080/announce",
                                 "games_compliance_ack": True}, tok)
check("S13 announce 填回环被拒", st == 400 or json.loads(bd).get("code") != 0,
      "http %s" % st)
st, bd = call("POST", "/admin/settings/smtp-test", {}, tok)
check("S14 SMTP 测试端点存在（未配置时报校验错而非 404）",
      st != 404, "http %s" % st)

print("\nroot 新口令（本脚本随机生成，仅测试实例）：%s" % NEW_PW)
if fails:
    print("==== 装机闸门 FAIL %d：%s ====" % (len(fails), "、".join(fails)))
    sys.exit(1)
print("==== 装机闸门全通过 ====")

# -*- coding: utf-8 -*-
"""资深 PT 深测公共层：登录/调用/断言输出"""
import os
import urllib.request, urllib.error, json, sys, time

# Windows 控制台默认 GBK：凡是打印到 GBK 编不出的字符（⇒ × ≥ → 之外的
# 那类）都会抛 UnicodeEncodeError 并**当场中断**整个闸门——后面所有判据
# 一条都不执行，FAIL 计数自然少报，看上去像「跑过了」。
# 批一百四十一实测：EP 跑到 EP2a 就断在 ⇒ 上，EP3~EP9 六段判据根本没跑。
# 统一把输出重配成 UTF-8（真编不出的字符用 replace 兜底），保证「红」能打完。
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except Exception:  # 老版本 Python / 已被包装过的流：能跑就好
        pass

# FLUX_API_BASE：打侧容器（宿主 :8080 常被别的进程占着，直连会拿到
# 别人的响应，判成败全错）。不设则沿用主栈。
BASE = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1")

def call(method, path, body=None, token=None, raw=False):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    data = json.dumps(body).encode() if body is not None else None
    try:
        r = urllib.request.urlopen(req, data, timeout=30)
        payload = r.read()
        if raw:
            return r.status, payload
        return r.status, json.loads(payload)
    except urllib.error.HTTPError as e:
        payload = e.read()
        if raw:
            return e.code, payload
        try:
            return e.code, json.loads(payload)
        except Exception:
            return e.code, {"raw": payload[:300].decode("utf-8", "replace")}

def call_h(method, path, body=None, token=None):
    """和 call() 同源，但把响应头也带回来。

    分页类判据要看 `X-Total-Count` / `X-Has-More`——那些信息刻意不进 body
    （消息中心三处前端把 data 当数组消费），所以判据只能从响应头取。
    返回 (status, payload_dict, headers_dict)。
    """
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    data = json.dumps(body).encode() if body is not None else None
    try:
        r = urllib.request.urlopen(req, data, timeout=30)
        status, payload, hdrs = r.status, r.read(), dict(r.headers)
    except urllib.error.HTTPError as e:
        status, payload, hdrs = e.code, e.read(), dict(e.headers)
    try:
        return status, json.loads(payload), hdrs
    except Exception:  # noqa: BLE001
        return status, {"raw": payload[:300].decode("utf-8", "replace")}, hdrs


def jwt_token(username="root"):
    """自铸职员令牌（HS256 + JWT_SECRET），口径同 `ci_m_ops_panels.py`。

    为什么需要：`e2e-smoke` 作业排在 `install_e2e.py` 之后，那一步的 S4 会把
    root 口令改成**随机** `NEW_PW`，而 seed 步的 `ON CONFLICT DO UPDATE` 只回写
    passkey/class_id、刻意不回写 pass_hash ⇒ 在这条作业里按口令登 root 必得
    400/2004「凭证无效」。
    为什么**不自动**回退到铸令牌：一次失败口令会给 `acctlock:root` 计一次，
    `login.rs` 里 5 次就把账号锁 15 分钟——闸门若每次悄悄回退，等于把站长的
    真 root 往锁定里推。所以只在显式开关下走这条路，本地仍练真口令。
    """
    from ci_reg_common import jwt, psql  # 惰性导入：两边互不依赖，不成环

    row = psql("SELECT id || ' ' || class_id FROM users "
               "WHERE username='%s'" % username.replace("'", "''"))
    if not row:
        return None
    uid, cls = row.split()[:2]
    return jwt(int(uid), cls=int(cls))


def login(username="root", pw="password123", tries=8):
    """带退避的登录。

    dev 实例上 `/auth/login` 会限流：几套闸门连着跑（每套要登好几次）之后
    直接返回无 token。后果比失败本身严重——放在 finally 里的兜底复原也要登录，
    于是兜底一起失败，脏数据留在原地。所以退避收在这一层，各闸门不必各自实现。

    `FLUX_GATE_JWT=1`：跳过口令，改用 `jwt_token()` 自铸（CI 里 root 口令已被
    装机步随机化，见 `jwt_token` 的注释）。只对默认 root 探针生效——各闸门自己
    造的探针号，口令是它们设的，必须继续走真登录。
    """
    if os.environ.get("FLUX_GATE_JWT") == "1" and username == "root":
        tok = jwt_token(username)
        if not tok:
            raise SystemExit(
                "FLUX_GATE_JWT=1 但铸不出令牌：查不到用户 %s，"
                "或 JWT_SECRET 既不在 env 也取不到容器 printenv" % username)
        return tok
    last = None
    for i in range(tries):
        s, r = call("POST", "/auth/login", {"username": username, "password": pw})
        if s == 200 and r.get("code") == 0:
            return r["data"]["token"]
        last = (s, r)
        if s != 429:
            break
        time.sleep(min(2 + i * 3, 20))
    raise SystemExit(
        "login failed for %s: %s %s —— 若这是 e2e-smoke 之类的 CI 作业：装机步"
        "已把 root 口令随机化，请给该作业设 FLUX_GATE_JWT=1（这里不自动回退，"
        "因为每次失败口令都在给 acctlock:root 计数，5 次锁 15 分钟）"
        % (username, last[0], last[1].get("message"))
    )

def ok(name, cond, detail=""):
    mark = "PASS" if cond else "FAIL"
    print("[%s] %s %s" % (mark, name, ("- " + str(detail)[:220]) if (detail and not cond) else ""))
    if not cond:
        FAILURES.append(name)
    return cond

FAILURES = []

def dig(resp, *keys):
    """Dig a nested key out of an API envelope without exploding on null.

    `X.get("data", {}).get(K)` only defaults when the KEY IS MISSING: a
    response of {"data": null} (rejected registrations do that) makes it return
    None and the NEXT .get raises AttributeError -- the gate dies before its main
    assertions while the finally-block cleanup still prints PASS, i.e. a run that
    looks green but measured nothing (batch 143, gates EB/EC). Callers write
    `dig(...) or []`.
    """
    cur = resp
    for k in keys:
        if not isinstance(cur, dict):
            return None
        cur = cur.get(k)
    return cur


def summary():
    # P-444：闸门经真实发布接口造的种必须自己收走。现网 213 条过审种里 203 条
    # 是历史夹具（`Audit.Pack.深测` 一个名字 93 条），会员端「最新」那一屏被
    # 测试行占满、公开列表又因存活过滤变成空——内容面被审计流程自己吃掉。
    # 回收挂在 summary()：登记是各闸门显式做的（note_torrent），删除集中一处。
    try:
        n = purge_noted_torrents()
        if n:
            print("[清理] 回收本轮闸门造的种子 %d 条" % n)
    except Exception as e:  # noqa: BLE001 收尾失败要出声，但不能吃掉断言账
        print("[清理失败] 种子夹具没回收干净：%s" % e)
        FAILURES.append("闸门造种回收失败")
    print()
    if FAILURES:
        print("== FAILURES (%d): %s" % (len(FAILURES), ", ".join(FAILURES)))
        sys.exit(1)
    print("== ALL PASS")


# ---- 闸门造种回收（P-444）--------------------------------------------
_CREATED_TORRENTS = []


def note_torrent(torrent_id):
    """登记「本轮我自己造出来的种子 id」，summary() 时统一回收。

    只登记显式传进来的 id，不按标题猜——按名字删会误伤真内容（同名 93 条里
    可能有站长自己起的名字）。
    """
    if torrent_id is None or torrent_id == "":
        return
    try:
        _CREATED_TORRENTS.append(int(torrent_id))
    except (TypeError, ValueError):
        pass


def purge_torrents(ids):
    """按 id 精确删种：先清 NO ACTION 子表，其余交给 CASCADE，回读残留。

    外键形状从 `pg_constraint` **运行时现取**而不是写死清单——写死的版本
    漏一张表就会把删除变成静默失败（实测只有 auto_review_log 真命中过 3 行）。
    残留不为 0 一律抛出，绝不让「以为清了」变成下一轮的红。
    """
    ids = sorted({int(i) for i in ids if i not in (None, "")})
    if not ids:
        return 0
    in_list = ",".join(str(i) for i in ids)
    blockers = psql_stdin(
        "SELECT c.conrelid::regclass::text, a.attname FROM pg_constraint c "
        "JOIN pg_attribute a ON a.attrelid=c.conrelid "
        "AND a.attnum=(c.conkey)[1] WHERE c.confrelid='torrents'::regclass "
        "AND c.contype='f' AND c.confdeltype='a' ORDER BY 1;")
    for ln in blockers.splitlines():
        p = ln.split("|")
        if len(p) != 2:
            continue
        psql_stdin("DELETE FROM %s WHERE %s IN (%s);" % (p[0], p[1], in_list))
    psql_stdin("DELETE FROM torrents WHERE id IN (%s);" % in_list)
    left = psql_stdin("SELECT count(*) FROM torrents WHERE id IN (%s);"
                      % in_list).splitlines()[0].strip()
    if left != "0":
        raise RuntimeError("种子夹具残留 %s 条（ids=%s）" % (left, ids))
    return len(ids)


def purge_noted_torrents():
    ids = list(dict.fromkeys(_CREATED_TORRENTS))
    del _CREATED_TORRENTS[:]
    return purge_torrents(ids)

def psql_stdin(sql: str, sep: str = "|") -> str:
    """经容器跑一段 SQL：**出错即抛**，不把错误吞成空串。

    为什么不能只信 returncode：psql 从 **stdin** 读 SQL 时，语句出错
    仍然返回 **0**（只有 -c 才按错误码返回 1）——实测
    `SELECT * FROM 不存在的表` 走 stdin 时 rc=0，ERROR 只出现在 stderr。
    闸门要是只看 returncode，SQL 写错了照样一路报绿：判据读到的全是
    空串/0 行，结论却是「通过」。这里统一两件事：
      ① 加 -v ON_ERROR_STOP=1（首个错误即中止，rc=3）；
      ② stderr 里出现 ERROR 一律当真错抛出去。
    """
    import subprocess

    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-v", "ON_ERROR_STOP=1", "-tA", "-F", sep],
        input=sql, capture_output=True, text=True, encoding="utf-8",
        errors="replace")
    err = (r.stderr or "").strip()
    if r.returncode != 0 or "ERROR" in err:
        raise RuntimeError("psql 失败：%s" % err[:300])
    return (r.stdout or "").strip()


def clear_register_rl():
    """闸门侧清掉**自己造出来的**注册额度占用。

    批一百五十三实测：这里原来只清 Redis 的 `rl:register-ip:*`，可 24 小时
    同 IP 限额根本不查 Redis——`register_gates::check_ip_limit` 数的是
    `reg_ip_signups` 表（sha3(ip) 摘要，窗口默认 24h，默认上限 2）。
    于是清 Redis 对它毫无作用：只要当天注册过 2 个探针号，第三支闸门
    建号必得 `同一网络环境 24 小时内最多注册 2 个账号`，`new_member`
    返回 None，下游全红在一片「未认证」上——红在前置而不是被测口径，
    极易误读成产品回归（#75 记的就是这件事，但当时归因成 Redis 频控）。

    所以要清两个面：Redis 的分钟级计数 + reg_ip_signups 的窗口计数。
    **只删自己造的探针号**（用户名/邮箱后缀匹配），不动 root 与真人账号，
    也不会削弱产品侧的防刷阈值——24h/2 是正当口径，产品侧一行不改。
    """
    import subprocess

    def psql(sql):
        return subprocess.run(
            ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
             "-d", "fluxtorrent", "-tAc", sql],
            capture_output=True, text=True, encoding="utf-8",
            errors="replace")

    # ① 分钟级 Redis 计数（登录/发信限流**不动**）
    cmd = ('redis-cli -a "$REDIS_PASSWORD" --no-auth-warning --scan '
           '--pattern "rl:register-ip:*" '
           '| xargs -r redis-cli -a "$REDIS_PASSWORD" --no-auth-warning del')
    subprocess.run(["docker", "exec", "flux-redis", "sh", "-c", cmd],
                   capture_output=True, text=True, encoding="utf-8")
    # ② 24h 窗口计数：只回收探针号留下的痕迹
    # 邮箱后缀各闸门不统一（aj/fc/as/bb/...），所以按「本地保留域」兜住：
    # *.local 不是可路由域，只可能是探针号；真人账号不会落在这里。
    # 探针邮箱域名实测有四类：<gate>.local（本仓闸门主流）、@localhost、
    # @audit.invalid（b_billing）、@example.com（若干闸门）。前两类靠保留域
    # 判定，后两类**不是**保留域，只能按「本仓闸门造号前缀」认——
    # 实测漏掉 audit.invalid 时 clear_register_rl() 静默空转（判据删除 0 行
    # 却不报错），b_billing 于是仍红在「两个测试号可用」。
    probe = (
        "SELECT id FROM users WHERE "
        "email LIKE '%%@%%.local' OR email LIKE '%%@localhost' "
        "OR email LIKE '%%@audit.invalid' OR email LIKE '%%@example.com' "
        "OR username LIKE 'deleted-%%'")
    out = psql("SELECT count(*) FROM reg_ip_signups WHERE user_id IN (%s)" % probe)
    if (out.stdout or "").strip() not in ("", "0"):
        psql("DELETE FROM reg_ip_signups WHERE user_id IN (%s)" % probe)


#
# 递归文本检索（纯 Python）。
# 为什么不用系统的 grep：Windows 上没有 grep 可执行文件，
# subprocess.run(["grep", ...]) 会抛 FileNotFoundError ⇒ 闸门带着 traceback
# 退出（rc=1）却**一条判据都没打**，看上去像产品坏了，其实是环境问题。
# 批一百四十七实测：hm_class_source / hs_lottery_pool / ht_seedbox_chain /
# hv_ads_liveness 四个闸门在 Windows 上全是这么崩的。
def grep_files(pattern, paths, root=".", suffix=None, ignore_case=False,
               fixed=False, skip_dirs=(".git", "node_modules", "target",
                                       ".next", "dist", ".turbo")):
    """返回命中的文件路径列表（相对 root 的路径，正斜杠）。"""
    import re
    rx = re.compile(re.escape(pattern) if fixed else pattern,
                    re.IGNORECASE if ignore_case else 0)
    if suffix and suffix.startswith("*"):
        suffix = suffix[1:]
    out = []
    for p in paths:
        base = os.path.join(root, p) if root else p
        if os.path.isfile(base):
            files = [base]
        else:
            files = []
            for dirpath, dirnames, filenames in os.walk(base):
                dirnames[:] = [d for d in dirnames if d not in skip_dirs]
                for fn in filenames:
                    if suffix and not fn.endswith(suffix):
                        continue
                    files.append(os.path.join(dirpath, fn))
        for f in files:
            try:
                with open(f, encoding="utf-8", errors="replace") as fh:
                    txt = fh.read()
            except OSError:
                continue
            if rx.search(txt):
                out.append(os.path.relpath(f, root).replace("\\", "/"))
    return out

def grep_count(pattern, paths, root=".", ignore_case=False, fixed=False,
                skip_dirs=(".git", "node_modules", "target", ".next", "dist",
                            ".turbo")):
    """返回命中的**行数**（等价 grep -c：命中文件数与行数都返回行数）。"""
    import re
    rx = re.compile(re.escape(pattern) if fixed else pattern,
                    re.IGNORECASE if ignore_case else 0)
    n = 0
    for f in grep_files(pattern, paths, root=root, ignore_case=ignore_case,
                         fixed=fixed, skip_dirs=skip_dirs):
        try:
            with open(os.path.join(root, f), encoding="utf-8",
                      errors="replace") as fh:
                n += sum(1 for ln in fh if rx.search(ln))
        except OSError:
            continue
    return n

def totp_now(secret_b32, skew=0):
    """按 RFC 6238 算 TOTP 码，与服务端 `twofa_http::totp_at` 同口径
    （HMAC-SHA1、30 秒窗口、动态截断、%1e6）。闸门要「真开 2FA」时才用它：
    `/me/2fa/setup` 回里就带 base32 密钥，返回 int 可直接进 JSON 的 code。
    `skew` 用来取相邻窗口的码（服务端容差 ±1 窗）。"""
    import base64
    import hashlib
    import hmac
    import time
    raw = base64.b32decode(secret_b32.upper().replace(" ", "").rstrip("="))
    step = int(time.time()) // 30 + skew
    digest = hmac.new(raw, step.to_bytes(8, "big"), hashlib.sha1).digest()
    off = digest[19] & 0x0F
    code = int.from_bytes(digest[off:off + 4], "big") & 0x7FFF_FFFF
    return code % 1_000_000

# -*- coding: utf-8 -*-
"""0312 音乐站 Logchecker 闸门（发种侧日志通道 / 落库 / 读口可见性）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8180/api/v1
    python scripts/pt_audit_g_logcheck.py

口径与 pt_audit_a/d/e 一致：直连接口 + psql 读回状态，判据全部落在
「只有新代码才可能通过」的行为上。本套**不测判分算法**（EAC/CUETools 的
扣分表要等真实日志样本固化后另加断言），这里测的是通道与闸门：

  1. policy=off 时日志 part 被忽略且**零落库**（存量站行为不变）
  2. policy=tag 时认不出的内容落 engine=unknown + log_score IS NULL
     ——「看不懂」绝不能变成「判成 0 分」
  3. GBK 码页日志经上传→落库→读口三段往返后中文轨道名仍是原字
  4. 多碟按提交顺序成 ordinal 0/1/…
  5. 体积上限是**拒收**而不是截断（截断会让判分依据「尾部行」凭空消失）
  6. require 只命中 logcheck_gate_site_types 里的站型
  7. 子资源读口走统一可见性口径：待审种对「非作者非管理」不可读，
     而对已过审种同一账号可读（正控，防「恒 404」假通过）
  8. 设定项在 settings_meta 里登记过（没登记 = 后台不渲染 = 假开关）
"""
import base64
import hashlib
import json
import os
import random
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, login, ok, summary,
)

# ---------------------------------------------------------------- 造种子
def benc(obj):
    if isinstance(obj, int):
        return b"i%de" % obj
    if isinstance(obj, bytes):
        return b"%d:%s" % (len(obj), obj)
    if isinstance(obj, str):
        return benc(obj.encode())
    if isinstance(obj, list):
        return b"l" + b"".join(benc(x) for x in obj) + b"e"
    if isinstance(obj, dict):
        out = b"d"
        for k in sorted(obj.keys()):
            out += benc(k) + benc(obj[k])
        return out + b"e"
    raise TypeError(type(obj))


def make_torrent(name=None, files=None, piece_length=32768):
    total = sum(s for _, s in files) if files else piece_length
    n = max(1, (total + piece_length - 1) // piece_length)
    info = {
        b"name": (
            name or "Audit.Log.%d" % random.randint(100000, 999999)
        ).encode(),
        b"piece length": piece_length,
        b"pieces": hashlib.sha1(b"piece").digest() * n,
        b"private": 1,
    }
    if files:
        info[b"files"] = [
            {b"length": s, b"path": [f.encode()]} for f, s in files
        ]
    else:
        info[b"length"] = total
    tor = {b"info": info, b"announce": b"http://127.0.0.1:7070/announce/x"}
    return benc(tor)


BOUNDARY = "----fluxlogcheck42"


def multipart(fields, parts):
    """fields: 文本元数据；parts: [(name, filename, bytes)] 二进制 part。"""
    out = b""
    for k, v in fields.items():
        if v is None:
            continue
        out += (
            "--%s\r\nContent-Disposition: form-data; name=\"%s\"\r\n\r\n%s\r\n"
            % (BOUNDARY, k, v)
        ).encode()
    for name, filename, content in parts:
        out += (
            "--%s\r\nContent-Disposition: form-data; name=\"%s\"; "
            "filename=\"%s\"\r\nContent-Type: text/plain\r\n\r\n"
            % (BOUNDARY, name, filename)
        ).encode()
        out += content + b"\r\n"
    out += ("--%s--\r\n" % BOUNDARY).encode()
    return out


def upload(tok, tor_bytes, logs=(), **fields):
    fields.setdefault("category_id", "1")
    parts = [("file", "audit.torrent", tor_bytes)]
    parts += [("log", fn, data) for fn, data in logs]
    body = multipart(fields, parts)
    req = urllib.request.Request(BASE + "/torrents", method="POST", data=body)
    req.add_header(
        "Content-Type", "multipart/form-data; boundary=%s" % BOUNDARY
    )
    req.add_header("Authorization", "Bearer " + tok)
    try:
        r = urllib.request.urlopen(req, timeout=60)
        return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        payload = e.read()
        try:
            return e.code, json.loads(payload)
        except Exception:
            return e.code, {"raw": payload[:300].decode("utf-8", "replace")}


# ---------------------------------------------------------------- 库与令牌
def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8",
    )
    if r.returncode != 0:
        raise SystemExit("psql 失败: %s" % (r.stderr or "")[:300])
    return (r.stdout or "").strip()


def set_kv(name, value):
    psql("INSERT INTO site_settings (name, value, descr, grp) "
         "VALUES ('%s','%s','probe','torrent') "
         "ON CONFLICT (name) DO UPDATE SET value = '%s'" % (name, value, value))
    got = psql("SELECT value FROM site_settings WHERE name = '%s'" % name)
    if got != value:
        raise SystemExit("设置没写进去: %s=%r 期望 %r" % (name, got, value))


def mint_token(uid, class_id, secret):
    """自签一枚「普通会员」令牌（可见性断言要用非作者非管理的视角）。"""
    hdr = base64.urlsafe_b64encode(b'{"alg":"HS256","typ":"JWT"}').rstrip(b"=")
    now = int(time.time())
    payload = json.dumps(
        {"sub": uid, "class_id": class_id, "iat": now, "exp": now + 600}
    ).encode()
    body = base64.urlsafe_b64encode(payload).rstrip(b"=")
    msg = hdr + b"." + body
    sig = base64.urlsafe_b64encode(
        hmac_sha(secret, msg)
    ).rstrip(b"=")
    return (msg + b"." + sig).decode()


def hmac_sha(secret, msg):
    import hmac
    return hmac.new(secret.encode(), msg, hashlib.sha256).digest()


def jwt_secret():
    env = os.path.join(os.path.dirname(__file__), "..", "docker", ".env")
    for line in open(env, encoding="utf-8"):
        if line.startswith("JWT_SECRET="):
            return line.split("=", 1)[1].strip().strip('"')
    raise SystemExit("docker/.env 里没有 JWT_SECRET")


# ---------------------------------------------------------------- 样本
def eac_like(text):
    return text.encode("utf-8")


# 认不出的内容：故意不含任何抓轨工具抬头（判分必须是 NULL 而不是 0）
NOT_A_LOG = (
    "Best In Show 2005 Tracklist\n"
    "01. Opening\n02. Main Theme\nEnd of list\n"
).encode()

# GBK/GB18030 码页的中文轨道名（判据是读回来的字面量，不是 HTTP 200）
GBK_NAME = "周杰伦 稻香"
GBK_LOG = (
    "EAC extraction logfile from 1. January 2020\r\n"
    "  Filename C:\\Rip\\01 - %s.flac\r\n" % GBK_NAME
).encode("gb18030")


# EAC 1.x 真实形态（2026-10-09 判分表重写后的打分回归样张）
def _eac_track(n, ar_line):
    return (
        "Track  {n}\r\n"
        "  Filename E:\\rip\\0{n}.flac\r\n"
        "  Peak level 99.9 %\r\n"
        "  Track quality 100.0 %\r\n"
        "  Test CRC 2AFB9E3F\r\n"
        "  Copy CRC 2AFB9E3F\r\n"
        "{ar_line}\r\n"
        "  Copy OK\r\n"
    ).format(n=n, ar_line=ar_line)


_HEAD = (
    "Exact Audio Copy V1.3 from 23. August 2016\r\n"
    "EAC extraction logfile from 5. February 2023, 14:30\r\n"
    "Used drive  : HL-DT-ST BD-RE WH14NS40\r\n"
    "Read mode               : Secure\r\n"
)
_TAIL = "No errors occurred\r\nEnd of status report\r\n"

# 完整证据链 + AR 全过 = 100
EAC_100 = (
    _HEAD
    + _eac_track(1, "  Accurately ripped (confidence 12) [2AFB9E3F]")
    + _eac_track(2, "  Accurately ripped (confidence 10) [2AFB9E3F]")
    + _TAIL
).encode("utf-8")

# 第 1 轨 AR 比对失败 + 第 2 轨没跑 AR：−30 −20 = 50
EAC_50 = (
    _HEAD
    + _eac_track(
        1, "  Cannot be verified as accurate (confidence 1) [00000000]")
    + _eac_track(2, "")
    + _TAIL
).encode("utf-8")

# Copy aborted → 直接 0（致命，不走扣分档）
EAC_0 = (
    _HEAD
    + _eac_track(1, "  Accurately ripped (confidence 12) [2AFB9E3F]")
    + "  Copy aborted\r\n"
    "End of status report\r\n"
).encode("utf-8")


# ---------------------------------------------------------------- 主流程
def main():
    tok = login()
    print("== 已登录 root ==")
    s, me = call("GET", "/me", token=tok)
    if s != 200 or (me.get("data") or {}).get("id") is None:
        raise SystemExit("令牌自检失败（后面所有否定式断言都会假通过）: %s %s" % (s, me))
    uid = me["data"]["id"]

    orig = {
        "logcheck_policy": psql("SELECT value FROM site_settings "
                                "WHERE name='logcheck_policy'") or "off",
        "logcheck_max_kib": psql("SELECT value FROM site_settings "
                                 "WHERE name='logcheck_max_kib'") or "1024",
        "site_type": psql("SELECT value FROM site_settings "
                          "WHERE name='site_type'") or "general",
    }
    print("   原设置 %s" % orig)
    made = []
    try:
        run_cases(tok, uid, made, orig)
    finally:
        # 兜底必须捕 BaseException：SystemExit 不是 Exception，
        # 早先版本让它逃掉 ⇒ 脏设置与脏种子留在原地
        try:
            for name, val in orig.items():
                set_kv(name, val)
            set_kv("logcheck_gate_site_types", "music,lossless")
            set_kv("logcheck_min_score", "100")
            for tid in made:
                subprocess.run(
                    ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
                     "-d", "fluxtorrent", "-c",
                     "DELETE FROM torrents WHERE id = %s" % tid],
                    capture_output=True, text=True,
                )
        except BaseException as e:  # noqa: BLE001
            print("!! 收尾异常（请手工核对 site_settings 与种子残留）: %r" % e)
    print("== 收尾已核对 ==")
    ok("探针种子已清干净",
       psql("SELECT count(*) FROM torrents WHERE name LIKE 'Audit.Logcheck.%'")
       == "0")
    ok("torrent_logs 无孤儿行",
       psql("SELECT count(*) FROM torrent_logs WHERE torrent_id IN "
            "(SELECT id FROM torrents WHERE name LIKE 'Audit.Logcheck.%')")
       == "0")
    bad = psql("SELECT count(*) FROM site_settings WHERE name LIKE "
               "'logcheck_%' AND value NOT IN ('off','1024','music,lossless',"
               "'100')")
    ok("收尾后设置值已复位", bad == "0", "site_settings 里仍有异常值: %s" % bad)
    summary()


def tid_of(r):
    return (r.get("data") or {}).get("id")


def run_cases(tok, uid, made, orig):
    # ---- 1. policy=off：日志被忽略，零落库
    set_kv("logcheck_policy", "off")
    tor = make_torrent()
    s, r = upload(tok, tor, logs=[("a.log", NOT_A_LOG)],
                  name="Audit.Logcheck.Off")
    tid = tid_of(r)
    if tid:
        made.append(tid)
    ok("off：发种仍成功", s == 200 and r.get("code") == 0, "%s %s" % (s, r))
    if not tid:
        raise SystemExit("off 用例就没发成功，后面的断言无法跑")
    stored = psql(
        "SELECT count(*) FROM torrent_logs WHERE torrent_id=%s" % tid)
    ok("off：零落库", stored == "0",
       "关了闸门还落库 = 存量站被新功能改了行为")

    # ---- 1b. 判分表回归（2026-10-09 重写）：100 / 50 / 0 三档真日志样张
    set_kv("logcheck_policy", "tag")
    tor = make_torrent()
    s, r = upload(
        tok, tor,
        logs=[("perfect.log", EAC_100), ("ar_fail.log", EAC_50),
              ("aborted.log", EAC_0)],
        name="Audit.Logcheck.Score")
    ok("判分样张：三碟发种成功", s == 200 and r.get("code") == 0,
       "%s %s" % (s, r))
    if s == 200 and r.get("code") == 0:
        made.append(r["data"]["id"])
        tid = r["data"]["id"]
        scores = psql(
            "SELECT ordinal || '|' || COALESCE(log_score::text,'NULL') "
            "FROM torrent_logs WHERE torrent_id=%s ORDER BY ordinal" % tid
        ).splitlines()
        ok("完美证据链 = 100 分", scores and scores[0].endswith("|100"),
           scores)
        ok("AR 失败+未验证 = 50 分（−30 −20 档）",
           len(scores) > 1 and scores[1].endswith("|50"), scores)
        ok("Copy aborted = 0 分（致命不走档）",
           len(scores) > 2 and scores[2].endswith("|0"), scores)

    # ---- 2/3/4. policy=tag：unknown 不出分 / GBK 往返 / 多碟顺序
    set_kv("logcheck_policy", "tag")
    tor = make_torrent()
    s, r = upload(
        tok, tor,
        logs=[("disc1.log", NOT_A_LOG), ("disc2.log", GBK_LOG)],
        name="Audit.Logcheck.Tag")
    ok("tag：带日志发种成功", s == 200 and r.get("code") == 0, "%s %s" % (s, r))
    tid = tid_of(r)
    if not tid:
        raise SystemExit("tag 用例发种失败，后续断言无法跑")
    made.append(tid)
    rows = psql("SELECT ordinal || '|' || engine || '|' || "
                "COALESCE(log_score::text,'NULL') FROM torrent_logs "
                "WHERE torrent_id=%s ORDER BY ordinal" % tid).splitlines()
    ok("tag：两碟两行且 ordinal 按提交顺序", len(rows) == 2 and
       rows[0].startswith("0|") and rows[1].startswith("1|"), rows)
    ok("认不出的内容 engine=unknown", "unknown" in (rows[0] if rows else ""),
       rows)
    ok("认不出的内容**不出分**（log_score IS NULL，不是 0）",
       psql("SELECT log_score IS NULL FROM torrent_logs WHERE torrent_id=%s "
            "AND ordinal=0" % tid) == "t",
       "把「看不懂」判成 0 分 = 假红，require 站会因此拒掉好种")
    ok("GBK 日志按内容识别为 eac",
       psql("SELECT engine FROM torrent_logs WHERE torrent_id=%s AND ordinal=1"
            % tid) == "eac")
    body1 = psql("SELECT body FROM torrent_logs "
                 "WHERE torrent_id=%s AND ordinal=1" % tid)
    ok("中文轨道名三段往返无损", GBK_NAME in body1)

    # 详情聚合与正文读口
    s, agg = call("GET", "/torrents/%s/aggregate" % tid, token=tok)
    logs = (agg.get("data") or {}).get("logs") or []
    # detail 必须自带：logs 为 [] 时是 falsy，原来那行会把失败原因吞掉
    ok("aggregate 带 logs 段（2 行）", len(logs) == 2,
       "http=%s code=%s msg=%s logs=%r" % (
           s, agg.get("code"), agg.get("message"), logs))
    ok("aggregate 的 logs 不含正文（正文懒加载）",
       all("body" not in l for l in logs), logs)
    s, one = call("GET", "/torrents/%s/logs/1" % tid, token=tok)
    ok("正文端点取到 GBK 原文",
       s == 200 and GBK_NAME in json.dumps(one, ensure_ascii=False), "%s %s" % (s, one))

    # ---- 5. 体积上限是拒收，不是截断
    set_kv("logcheck_max_kib", "64")
    big = ("EAC extraction logfile from 1. January 2020\r\n" * 4000).encode()
    s, r = upload(tok, make_torrent(), logs=[("big.log", big)],
                  name="Audit.Logcheck.Big")
    if s == 200 and r.get("code") == 0:
        made.append(r["data"]["id"])
    ok("超上限日志被拒（400）", s == 400, "%s %s" % (s, r))
    ok("拒因写的是体积上限而不是别的",
       "超过上限" in (r.get("message") or ""), r.get("message"))
    ok("被拒的这发没有留下待审残种",
       psql("SELECT count(*) FROM torrents WHERE name='Audit.Logcheck.Big'") == "0",
       "INSERT 之后才报错 = 判重永久锁死这条内容（0288 的根因）")
    set_kv("logcheck_max_kib", orig["logcheck_max_kib"])

    # ---- 6. require 只命中 gate 清单里的站型
    set_kv("logcheck_policy", "require")
    set_kv("logcheck_gate_site_types", "music,lossless")
    set_kv("site_type", "general")
    s, r = upload(tok, make_torrent(), name="Audit.Logcheck.GenNoLog")
    if s == 200 and r.get("code") == 0:
        made.append(r["data"]["id"])
    ok("require + 清单外站型：不交日志照样发得出", s == 200,
       "%s %s" % (s, r))
    set_kv("site_type", "lossless")
    s, r = upload(tok, make_torrent(), name="Audit.Logcheck.NeedLog")
    if s == 200 and r.get("code") == 0:
        made.append(r["data"]["id"])
    ok("require + lossless：不交日志被拒", s == 400, "%s %s" % (s, r))
    ok("拒因直指缺日志而不是别的校验",
       "必须提交抓轨日志" in (r.get("message") or ""), r.get("message"))
    ok("闸门拒发同样不留残种",
       psql("SELECT count(*) FROM torrents "
            "WHERE name='Audit.Logcheck.NeedLog'") == "0")
    set_kv("site_type", orig["site_type"])

    # ---- 7. 读口的可见性口径（负例 + 正控，缺一不可）
    set_kv("logcheck_policy", "tag")
    tor = make_torrent()
    s, r = upload(tok, tor, logs=[("x.log", GBK_LOG)],
                  name="Audit.Logcheck.Visibility")
    tid2 = tid_of(r)
    if not tid2:
        raise SystemExit("可见性用例发种失败: %s %s" % (s, r))
    made.append(tid2)
    other = psql("SELECT id || '/' || class_id FROM users "
                 "WHERE id <> %d AND class_id < 90 AND status < 2 "
                 "ORDER BY id LIMIT 1" % uid)
    if "/" not in other:
        raise SystemExit("库里找不到「非管理非作者」的对照账号，可见性断言跑不了")
    ouid, oclass = (int(x) for x in other.split("/"))
    otok = mint_token(ouid, oclass, jwt_secret())
    s, _ = call("GET", "/me", token=otok)
    if s != 200:
        raise SystemExit("自签普通会员令牌没通过自检（JWT_SECRET 或 claims 不对）")
    psql("UPDATE torrents SET approval_status = 0 WHERE id = %s" % tid2)
    s, _ = call("GET", "/torrents/%s/logs/0" % tid2, token=otok)
    ok("待审种的日志对「非作者非管理」不可读", s == 404, "%s" % s)
    s, _ = call("GET", "/torrents/%s/logs/0" % tid2, token=tok)
    ok("同一枚待审种，作者本人可读（正控）", s == 200, "%s" % s)
    psql("UPDATE torrents SET approval_status = 1 WHERE id = %s" % tid2)
    s, _ = call("GET", "/torrents/%s/logs/0" % tid2, token=otok)
    ok("过审后同一普通账号可读（正控：证明上面不是恒 404）", s == 200, "%s" % s)

    # ---- 8. 设定项登记（没进 settings_meta = 后台根本不渲染）
    s, schema = call("GET", "/admin/settings/schema", token=tok)
    fields = {}
    for grp in (schema.get("data") or {}).get("groups") or []:
        for card in grp.get("cards") or []:
            for f in card.get("fields") or []:
                fields[f["name"]] = f
    for key in ("logcheck_policy", "logcheck_min_score", "logcheck_max_kib",
                "logcheck_gate_site_types"):
        ok("后台设置页能看到 %s" % key, key in fields, "settings_meta 未登记")
    pol = fields.get("logcheck_policy") or {}
    ok("policy 是 enum 且有 3 个选项（否则后台渲染成空下拉）",
       pol.get("type") == "enum" and len(pol.get("options") or []) == 3,
       pol)


if __name__ == "__main__":
    main()

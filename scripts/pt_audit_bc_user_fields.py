# -*- coding: utf-8 -*-
"""闸门 BC：用户自定义字段全链（批三十九）。

「支持任何类型 PT 站」是本站的产品定位，而这句话的落地物就是这张
`user_field_defs` + `user_field_values`：站长自己定义字段（六类型）、注册页收集、
用户在 usercp 改、公开档案展示、运营代改、删除时连带清值。**此前零覆盖**
（`grep -l user-fields scripts/*.py` 只命中 legacy 的 admin_e2e/healthcheck，
pt_audit_* 一条都没有；线上 `user_field_defs` 目前 0 行 = 这功能从没被真实
走过一遍）。

判据一律「真发请求 → 真读库 + 真读注册结果」，SQL 字面量全 ASCII
（中文只从接口/库里读出来比对）：
  S1 定义卫生  key 白名单、六类型、options 形状、visibility、module_key 未知、
              重复 key、type 不可改；**label / option value 的长度按「字」判**
              （文案写「字符」，代码 `str::len()` 数字节 ⇒ 中文 34 字就撞 100
              上限，站方连「毕业学校及专业名称」这种名字都嫌长）
  S2 值形状    text/number/select/multiselect/date/bool 逐档正控 + 反控；
              **text 值同样按字节**（200 个汉字 = 600 字节当场 400）；
              未知 key 静默跳过 vs 如实回报；一次提交的 key 条数上限
  S3 注册链    必填注册字段挂模块：模块**开着**时必须出现在表单里且要求填；
              模块**关着**时表单里没有它 —— 而 register 的校验循环此前
              **不查模块**，于是「表单看不见、后端却硬要」= 注册被永久锁死
  S4 停用/删除 停用后本人读写与后台代改的口径；有值字段删除要 confirm，
              且删了 def 之后 values 必须真的随 CASCADE 清空
  S5 可见性    public 进公开档案、private 不进；运营侧一次看全（含停用）
  S6 审计      字段定义 update/delete 的 audit_log 必须记得到**哪个 key**
              （旧写法 ref=None ⇒ 只留下「有人动过字段定义」，出事查不到是哪条）
  S7 收尾      探针 defs 全删（CASCADE 清 values）、探针号墓碑、
              `user_field_defs` 回到跑前状态

跑法（仓库根目录，需本地栈在跑；会临时建两条必填注册字段，独占跑）：
    PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bc_user_fields.py
"""
import os
import re
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
KEYS = []          # 本轮建的字段 key
UIDS = []          # 本轮建的探针成员
CORE = ["bc_text", "bc_num", "bc_sel", "bc_multi", "bc_date", "bc_bool",
        "bc_priv", "bc_on", "bc_off"]   # 地基：少了任何一条后面全是空判
MOD_ON = "forums"      # 现网开着
MOD_OFF = "showcase"   # 现网 module_showcase = no
ZH30 = "字" * 30       # 30 汉字 = 90 字节（label 上限说 1-50 字符）
ZH200 = "汉字" * 200   # 200 字 = 600 字节（text 上限说 ≤500 字符）


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % (r.stderr or "")[:200])
    return r.stdout.strip()


def note(msg):
    print("   %s" % (msg,))


def cleanup():
    psql("UPDATE users SET status = 2 WHERE id IN (%s)"
         % (",".join([str(u) for u in UIDS]) or "-1"))
    psql("DELETE FROM user_field_defs WHERE key = ANY(ARRAY[%s])"
         % ",".join("'%s'" % k for k in KEYS))
    note("[cleanup] 探针字段 %d 条已删（values 随 CASCADE）、探针号 %s 已墓碑；"
         "defs 现存 %s 行（跑前为 0）"
         % (len(KEYS), UIDS, psql("SELECT count(*) FROM user_field_defs")))


import atexit  # noqa: E402
atexit.register(cleanup)


def add_field(key, ftype="text", **kw):
    body = {"type": ftype, "label": kw.get("label", "Label " + key),
            "required": kw.get("required", False),
            "visibility": kw.get("visibility", "public"),
            "show_on_register": kw.get("show_on_register", False),
            "options": kw.get("options", []),
            "sort": kw.get("sort", 100),
            "enabled": kw.get("enabled", True),
            "module_key": kw.get("module_key", "")}
    KEYS.append(key)
    return call("POST", "/admin/user-fields/%s" % key, body, token=ROOT)


def opts2():
    return [{"value": "a", "label": "A"}, {"value": "b", "label": "B"}]


def register(name, fields=None, want_invite=True):
    clear_register_rl()
    time.sleep(6)
    codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                   token=ROOT)[1].get("data") or {}).get("codes")) or []
    cid = None
    code = ""
    if codes:
        code, cid = codes[0]["code"], codes[0]["id"]
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = [int(x) for x in re.findall(r"-?\d+", str(cap.get("question")))]
    s, r = call("POST", "/auth/register", {
        "username": name, "email": "%s@bc.local" % name,
        "password": "password123", "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": (nums[0] + nums[1]) if len(nums) > 1 else 1,
        "captcha_token": "", "fields": fields or {}}, token=None)
    if want_invite and cid:
        psql("DELETE FROM invites WHERE id = %d" % cid)
    return s, r


print("== S1 定义卫生 ==")
s, r = call("POST", "/admin/user-fields/BC-UP", {"type": "text",
                                                "label": "x"}, token=ROOT)
ok("S1 非法 key 必须 400 且点到 key 规则（大写/连字符不收）",
   s == 400 and "key" in str((r or {}).get("message", "")),
   "%s %s" % (s, str((r or {}).get("message", ""))[:70]))
s, r = call("POST", "/admin/user-fields/bc_blob", {"type": "blob",
                                                  "label": "x"}, token=ROOT)
msg = str((r or {}).get("message", ""))
ok("S1 未知类型必须 400 且点到六类型全集",
   s == 400 and "multiselect" in msg, "%s %s" % (s, msg[:70]))
s, r = call("POST", "/admin/user-fields/bc_sel0", {"type": "select",
                                                   "label": "x",
                                                   "options": []},
            token=ROOT)
ok("S1 select 不给 options 必须 400 且点到「options」",
   s == 400 and "options" in str((r or {}).get("message", "")),
   "%s %s" % (s, str((r or {}).get("message", ""))[:70]))
s, r = call("POST", "/admin/user-fields/bc_sel1",
            {"type": "select", "label": "x",
             "options": [{"value": 1, "label": "A"}]}, token=ROOT)
ok("S1 options 的 value 给数字必须 400（若放过，allowed_values 会空集 ⇒"
   "该字段永远填不进）",
   s == 400 and "value" in str((r or {}).get("message", "")),
   "%s %s" % (s, str((r or {}).get("message", ""))[:70]))
s, r = add_field("bc_badvis", visibility="everybody")
ok("S1 visibility 非 public/private 必须 400",
   s == 400 and "visibility" in str((r or {}).get("message", "")),
   "%s %s" % (s, str((r or {}).get("message", ""))[:70]))
KEYS.pop()
s, r = add_field("bc_mod", module_key="nonexist_mod")
ok("S1 未知模块键必须 400 且点到「未知模块键」",
   s == 400 and "未知模块键" in str((r or {}).get("message", "")),
   "%s %s" % (s, str((r or {}).get("message", ""))[:80]))
KEYS.pop()
s, r = add_field("bc_text")
ok("S1 正控：一条合法 text 字段可建（200 + added）",
   s == 200 and (r or {}).get("code") == 0, "%s %s" % (s, str(r)[:70]))
s, r = add_field("bc_text")
ok("S1 重复 key 必须 400 且点到「已存在」（不是 UNIQUE 冲突变 500）",
   s == 400 and "已存在" in str((r or {}).get("message", "")),
   "%s %s" % (s, str((r or {}).get("message", ""))[:70]))
KEYS.pop()
s, r = add_field("bc_label", label=ZH30)
ok("S1 30 个汉字的字段名必须收下：文案写「1-50 字符」而代码数 UTF-8 字节"
   "（P-137a），红线现场见首跑",
   s == 200 and (r or {}).get("code") == 0,
   "%s %s" % (s, str((r or {}).get("message", ""))[:80]))
if s != 200:
    KEYS.pop()
s, r = add_field("bc_opt", "select", options=[
    {"value": "选" * 60, "label": "L"}])
ok("S1 option value 60 个汉字（180 字节）必须收下（上限写的是 1-100 字符，"
   "P-137b）", s == 200 and (r or {}).get("code") == 0,
   "%s %s" % (s, str((r or {}).get("message", ""))[:80]))
if s != 200:
    KEYS.pop()
s, r = call("PUT", "/admin/user-fields/bc_text",
            {"type": "number", "label": "L", "visibility": "public",
             "sort": 100, "enabled": True, "options": [], "required": False,
             "show_on_register": False}, token=ROOT)
ok("S1 改字段时必须挡住「换类型」（存量值形状会失配）且点到「不可修改」",
   s == 400 and "不可修改" in str((r or {}).get("message", "")),
   "%s %s" % (s, str((r or {}).get("message", ""))[:70]))
s, r = call("PUT", "/admin/user-fields/bc_nosuch",
            {"type": "text", "label": "L", "visibility": "public",
             "sort": 100, "enabled": True, "options": [], "required": False,
             "show_on_register": False}, token=ROOT)
ok("S1 改不存在的字段必须 400 且点到「不存在」",
   s == 400 and "不存在" in str((r or {}).get("message", "")),
   "%s %s" % (s, str((r or {}).get("message", ""))[:70]))

print("== S1b 建齐六类型 + 两条挂模块的必填注册字段 ==")
for k, t, extra in (("bc_num", "number", {}),
                    ("bc_sel", "select", {"options": opts2()}),
                    ("bc_multi", "multiselect", {"options": opts2()}),
                    ("bc_date", "date", {}), ("bc_bool", "bool", {}),
                    ("bc_priv", "text", {"visibility": "private"}),
                    ("bc_on", "text", {"required": True, "sort": 1,
                                       "show_on_register": True,
                                       "module_key": MOD_ON}),
                    ("bc_off", "text", {"required": True, "sort": 2,
                                        "show_on_register": True,
                                        "module_key": MOD_OFF})):
    s, r = add_field(k, t, **extra)
    if s != 200:
        ok("建字段 %s(%s) 必须成功（地基）" % (k, t), False,
           "%s %s" % (s, str((r or {}).get("message", ""))[:90]))
ok("S1b 六类型 + private + 两条挂模块必填注册字段全部建好（地基）"
   "（缺 %s）"
   % ([k for k in CORE if k not in
       psql("SELECT string_agg(key, ',') FROM user_field_defs").split(",")],),
   all(k in psql("SELECT string_agg(key, ',') FROM user_field_defs").split(",")
       for k in CORE), "defs=%s" % psql(
           "SELECT string_agg(key, ',') FROM user_field_defs"))
note("module_showcase 现值=%s（闸门不写站点设定，只借现成的开/关做口径对照）"
     % psql("SELECT value FROM site_settings WHERE name ="
            " 'module_showcase'"))

print("== S2 值形状（PUT /me/fields）==")
s, r = call("PUT", "/me/fields", {"values": {"bc_text": ZH200}}, token=ROOT)
ok("S2 text 值 200 个汉字（600 字节）必须收下：校验器 doc 写「≤500 字符」而"
   "代码 `s.len()` 数字节 ⇒ 中文只能填 166 字（P-137c）",
   s == 200 and ((r or {}).get("data") or {}).get("saved") == 1,
   "%s %s" % (s, str(r)[:90]))
bad = [("bc_num", "320", "number"), ("bc_sel", "z", "选项"),
       ("bc_multi", ["a", "z"], "选项"), ("bc_date", "2024-13-45", "YYYY"),
       ("bc_bool", "true", "bool")]
for key, val, kw in bad:
    s, r = call("PUT", "/me/fields", {"values": {key: val}}, token=ROOT)
    ok("S2 反控 %s 收 %r 必须 400 且点到「%s」" % (key, val, kw),
       s == 400 and kw in str((r or {}).get("message", "")),
       "%s %s" % (s, str((r or {}).get("message", ""))[:80]))
s, r = call("PUT", "/me/fields", {"values": {"bc_num": 320, "bc_sel": "a",
                                             "bc_multi": ["a", "b"],
                                             "bc_date": "2024-05-01",
                                             "bc_bool": True}}, token=ROOT)
ok("S2 正控：五档合法值一次保存 saved=5",
   s == 200 and ((r or {}).get("data") or {}).get("saved") == 5,
   "%s %s" % (s, str(r)[:90]))
s, r = call("PUT", "/me/fields", {"values": {"bc_ghost": "x"}}, token=ROOT)
d = (r or {}).get("data") or {}
ok("S2 不存在的 key 不能与成功混在一起：响应必须区分 saved / skipped"
   "（P-138；修复前只回 saved，用户以为写进去了）",
   s == 200 and d.get("skipped") == 1 and d.get("saved") == 0,
   "本轮实测 %s" % (str(r)[:90],))
flood = {"k%d" % i: "v" for i in range(300)}
s, r = call("PUT", "/me/fields", {"values": flood}, token=ROOT)
ok("S2 一次提交 300 个 key 必须拒（每条一次库查询，无上限＝可被用来打库，"
   "P-139），实测 http=%s" % s,
   s == 400 and "字段" in str((r or {}).get("message", "")),
   "%s %s" % (s, str((r or {}).get("message", ""))[:80]))
s, r = call("PUT", "/me/fields", {"values": {"bc_on": None}}, token=ROOT)
ok("S2 必填字段不允许清空且点到字段名",
   s == 400 and "必填" in str((r or {}).get("message", "")),
   "%s %s" % (s, str((r or {}).get("message", ""))[:80]))

print("== S3 注册链：必填字段挂模块，关着就不该要求 ==")
s, r = call("GET", "/register-fields", token=None)
keys_seen = [x.get("key") for x in ((r or {}).get("data") or [])]
ok("S3 表单只下发**开着**的模块的注册字段（含 bc_on、不含 bc_off）",
   "bc_on" in keys_seen and "bc_off" not in keys_seen, "%s" % (keys_seen,))
s, r = register("bc%d" % TAG, {"bc_on": "T1"})
uid = psql("SELECT id FROM users WHERE username = 'bc%d'" % TAG)
if uid.isdigit():
    UIDS.append(int(uid))
ok("S3 正控：填上可见的必填字段就能注册成功（http=%s）" % s,
   s == 200 and (r or {}).get("code") == 0, "%s %s" % (s, str(r)[:90]))
s2, r2 = register("bc%dz" % TAG, {"bc_on": "T2"})
uid2 = psql("SELECT id FROM users WHERE username = 'bc%dz'" % TAG)
if uid2.isdigit():
    UIDS.append(int(uid2))
ok("S3 模块关着的必填字段（表单里看不见）**不许**挡注册（P-140；"
   "修复前实测 400「『Label bc_off』为必填字段」= 注册被永久锁死）",
   s2 == 200 and (r2 or {}).get("code") == 0,
   "本轮实测 http=%s %s" % (s2, str((r2 or {}).get("message", ""))[:60]))
if uid.isdigit():
    vals = psql("SELECT value FROM user_field_values WHERE user_id = %s"
                " AND field_key = 'bc_on'" % uid)
    ok("S3 注册时填的字段值真的落库（value=%s）" % (vals,), vals == '"T1"',
       "%s" % (vals,))

print("== S4 停用与删除 ==")
# 先给 bc_text 写一条 ASCII 值：S4 的「有值字段删除」前提不能依赖
# P-137c 那条判据（红的时候写不进，会把删除判据一起带崩）
s, r = call("PUT", "/me/fields", {"values": {"bc_text": "has-value"}},
            token=ROOT)
ok("S4 地基：给 bc_text 写一条 ASCII 值要成功（saved=1）",
   s == 200 and ((r or {}).get("data") or {}).get("saved") == 1,
   "%s %s" % (s, str(r)[:80]))
s, r = call("PUT", "/admin/user-fields/bc_sel",
            {"type": "select", "label": "L", "required": False,
             "visibility": "public", "show_on_register": False,
             "options": opts2(), "sort": 100, "enabled": False,
             "module_key": ""}, token=ROOT)
ok("S4 停用字段（enabled=false）改定义成功", s == 200
   and (r or {}).get("code") == 0, "%s %s" % (s, str(r)[:80]))
s, mine = call("GET", "/me/fields", token=ROOT)
mk = [x.get("key") for x in ((mine or {}).get("data") or [])]
ok("S4 停用后本人读写面不再下发该字段（实测 %s）" % ("bc_sel" in mk,),
   "bc_sel" not in mk, "%s" % (mk,))
s, r = call("PUT", "/admin/users/1/fields", {"values": {"bc_sel": "a"}},
            token=ROOT)
ok("S4 后台代改碰停用字段必须显式 400 且点到「停用」（静默跳过会让运营"
   "以为改成功）", s == 400 and "停用" in str((r or {}).get("message", "")),
   "%s %s" % (s, str((r or {}).get("message", ""))[:80]))
s, r = call("POST", "/admin/user-fields/bc_text/delete", {}, token=ROOT)
msg = str((r or {}).get("message", ""))
ok("S4 有值的字段删除必须要求 confirm 并报出已填人数",
   s == 400 and "confirm_drop_values" in msg, "%s %s" % (s, msg[:90]))
n_val = int(psql("SELECT count(*) FROM user_field_values WHERE field_key ="
                " 'bc_text'"))
s, r = call("POST", "/admin/user-fields/bc_text/delete",
            {"confirm_drop_values": True}, token=ROOT)
left = int(psql("SELECT count(*) FROM user_field_values WHERE field_key ="
                " 'bc_text'"))
ok("S4 confirm 后删除，且 values 真的随 CASCADE 清空（删前 %d 行、删后 %d 行）"
   % (n_val, left), s == 200 and n_val >= 1 and left == 0,
   "%s before=%d after=%d" % (str(r)[:70], n_val, left))
if "bc_text" in KEYS:
    KEYS.remove("bc_text")

print("== S5 可见性与运营侧一次看全 ==")
s, r = call("PUT", "/me/fields", {"values": {"bc_priv": "SECRET-ONLY"}},
            token=ROOT)
ok("S5 本人可写 private 字段（saved=1）",
   s == 200 and ((r or {}).get("data") or {}).get("saved") == 1,
   "%s %s" % (s, str(r)[:70]))
pub = str(call("GET", "/users/1", token=ROOT)[1] or {})
# 读自己的档案也要挡 private：public_fields_for 只看 visibility，
# 不给本人开后门（本人要看在 usercp，公开页就是公开页）
ok("S5 private 字段值**不得**出现在公开档案（GET /users/1 里查到"
   " SECRET-ONLY = 泄漏）", "SECRET-ONLY" not in pub,
   "%s" % (pub[-160:],))
s, r = call("PUT", "/me/fields", {"values": {"bc_num": 1234}}, token=ROOT)
prof = str(call("GET", "/users/1", token=ROOT)[1] or {})
ok("S5 public 字段值要出现在公开档案（1234 没出现 = 展示了个空）",
   "1234" in prof, "%s" % (prof[-160:],))
s, a = call("GET", "/admin/users/1/fields", token=ROOT)
rows = (a or {}).get("data") or []
kk = {x.get("key"): x for x in rows}
ok("S5 运营侧一次看全：含 private 与停用字段（实测 %d 行，"
   "bc_priv=%s bc_sel.enabled=%s）"
   % (len(rows), (kk.get("bc_priv") or {}).get("visibility"),
      (kk.get("bc_sel") or {}).get("enabled")),
   "bc_priv" in kk and kk["bc_priv"].get("visibility") == "private"
   and kk.get("bc_sel", {}).get("enabled") is False, "%s" % (list(kk),))

print("== S6 字段定义的审计要记得到哪个 key ==")
# audit_log 有防篡改触发器（audit_log_chain 禁 DELETE）⇒ 只数本轮新建的行，
# 按 id 上界比对，不删不改历史
before_max = psql("SELECT COALESCE(max(id),0) FROM audit_log")
s, r = call("PUT", "/admin/user-fields/bc_num",
            {"type": "number", "label": "L2", "required": False,
             "visibility": "public", "show_on_register": False,
             "options": [], "sort": 111, "enabled": True,
             "module_key": ""}, token=ROOT)
ref = psql("SELECT COALESCE(ref::text,'__NULL__') FROM audit_log WHERE id > %s"
           " AND action = 'user_field_update' ORDER BY id DESC LIMIT 1"
           % before_max)
ok("S6 审计行要能从 ref 里读出被改的是哪个字段（实测 ref=%s）——旧写法"
   " ref=None，只留下「有人动过字段定义」（P-141）" % (ref,),
   s == 200 and "bc_num" in ref, "%s %s" % (s, ref))

print("== S7 收尾 ==")
for k in ("bc_num", "bc_sel", "bc_multi", "bc_date", "bc_bool", "bc_priv",
          "bc_on", "bc_off", "bc_label", "bc_opt"):
    call("POST", "/admin/user-fields/%s/delete" % k,
         {"confirm_drop_values": True}, token=ROOT)
psql("UPDATE users SET status = 2 WHERE id IN (%s)"
     % (",".join([str(u) for u in UIDS]) or "-1"))
ok("S7 探针字段全删（defs 现存 %s 行，跑前 0 行）"
   % psql("SELECT count(*) FROM user_field_defs"),
   psql("SELECT count(*) FROM user_field_defs") == "0")
ok("S7 values 无残留（%s 行）"
   % psql("SELECT count(*) FROM user_field_values"),
   psql("SELECT count(*) FROM user_field_values") == "0")
ok("S7 探针号已墓碑（活着的 %d 个）"
   % len([u for u in UIDS if psql("SELECT status FROM users WHERE id = %s"
                                  % u) != "2"]),
   not [u for u in UIDS if psql("SELECT status FROM users WHERE id = %s"
                                % u) != "2"], "%s" % (UIDS,))
ok("S7 站点设定未写一个字（module_showcase 仍=%s、module_forums 仍=%s）"
   % (psql("SELECT value FROM site_settings WHERE name = 'module_showcase'"),
      psql("SELECT value FROM site_settings WHERE name = 'module_forums'")),
   psql("SELECT value FROM site_settings WHERE name = 'module_showcase'")
   == "no")
summary()

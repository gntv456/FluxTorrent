# -*- coding: utf-8 -*-
"""0317/0318 站型包切换安全闸门（H1-H5：key 身份 / brand 守卫 / 覆盖位 / diff 分类段）。

跑法：
    export FLUX_API_BASE=http://127.0.0.1:8080/api/v1
    python scripts/pt_audit_h_packswitch.py

口径与 pt_audit_a-g 一致：直连接口 + psql 读回状态，判据全部落在
「只有新代码才可能通过」的行为上：

  H1  切站型后，在用分类的**名字不被偷换**（key 化：general 的电影 55 枚种子，
      切 education 后仍叫电影；分类表追加了新行而不是原地改名）
  H2  diff 预览含分类段：category_new / category_rename(带 torrents) /
      category_inactive 至少出现 new；键值行仍是旧格式兼容
  H3  brand 为空的包 apply 后 site_name 不被清空
  H4  设置面手工改过的模块键在再次 apply 后保持手工值（覆盖位生效）
  H5  台账 changes 里能读到分类段行（预览=记录同源）
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402

# 演练库容器（--drill 起的 55432 实例）；默认打主栈库
PG = ["docker", "exec", os.environ.get("FLUX_PG_CONTAINER", "flux-postgres"),
      "psql", "-U", "flux", "-d",
      os.environ.get("FLUX_PG_DB", "fluxtorrent"), "-t", "-A", "-c"]


def psql(sql):
    r = subprocess.run(PG + [sql], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit("psql failed: " + r.stderr[:200])
    return r.stdout.strip()


def get_pack(token, code):
    s, r = call("GET", "/admin/site-type-packs", token=token)
    assert s == 200 and r.get("code") == 0, r
    for p in r["data"]:
        if p["code"] == code:
            return p
    raise SystemExit("pack not found: " + code)


def main():
    # 演练库的 root 口令与主栈不同（首次起库会强制改临时密码）：
    # 先按主栈口径登，失败则回落演练口径（可用 FLUX_DRILL_PW 覆盖）
    try:
        token = login()
    except SystemExit:
        token = login(
            "root",
            os.environ.get("FLUX_DRILL_PW", "Audit#Passw0rd!"),
        )

    # ---- 前置快照（站点现状） ----
    site_type0 = psql("SELECT value FROM site_settings WHERE name='site_type'")
    site_name0 = psql(
        "SELECT COALESCE((SELECT value FROM site_settings "
        "WHERE name='site_name'),'')"
    )
    cats0 = {
        row.split("|")[0]: row.split("|")[1]
        for row in psql(
            "SELECT id || '|' || name FROM categories"
        ).splitlines()
        if row
    }
    torrents0 = psql("SELECT count(*) FROM torrents")
    print(f"# 前置：site_type={site_type0} torrents={torrents0} "
          f"cats={len(cats0)} site_name={site_name0!r}")

    # ---- H2：diff 预览含分类段 ----
    target = "education" if site_type0 != "education" else "game"
    s, r = call("POST", "/admin/site-type-packs/diff",
                {"code": target}, token=token)
    ok("H2 diff 200", s == 200 and r.get("code") == 0, r)
    changes = r.get("data", {}).get("changes", [])
    kinds = {c["key"] for c in changes}
    ok("H2 diff 含 category_new 段",
       "category_new" in kinds, sorted(kinds)[:8])
    ok("H2 diff 键值行兼容（site_type 在列）",
       any(c["key"] == "site_type" for c in changes))

    # ---- H4 前置：登记一个手工覆盖位（直接造表行——设置面路径已在
    #      groups.rs 落同样登记，这里造数据等价） ----
    probe_mod = "gomoku"
    cur_val = psql(
        "SELECT value FROM site_settings WHERE name='module_%s'" % probe_mod
    )
    psql("INSERT INTO pack_module_overrides (module_key) VALUES ('%s') "
         "ON CONFLICT (module_key) DO NOTHING" % probe_mod)

    # ---- apply 往返 ----
    s, r = call("POST", "/admin/site-type-packs/apply",
                {"code": target, "mode": "merge"}, token=token)
    ok("apply 200", s == 200 and r.get("code") == 0, r)

    # H1：老分类名不被偷换（按 key 追加新行，旧行名字保持）
    cats1 = {
        row.split("|")[0]: row.split("|")[1]
        for row in psql(
            "SELECT id || '|' || name FROM categories"
        ).splitlines()
        if row
    }
    stolen = [
        (cid, cats0[cid], cats1[cid])
        for cid in cats0
        if cid in cats1 and cats0[cid] != cats1[cid]
    ]
    ok("H1 在用分类名不被偷换", not stolen, stolen[:5])
    # 在用分类的种子语义完好：general 电影类（55 枚）若在切前存在，切后名字仍在
    if site_type0 == "general":
        ok("H1 general 电影分类仍在",
           any(n == "电影" for n in cats1.values()),
           sorted(set(cats1.values()))[:12])
    n_after = psql("SELECT count(*) FROM categories")
    ok("H1 分类表按 key 追加（行数不减）",
       int(n_after) >= len(cats0), (len(cats0), n_after))
    ok("H1 分类行 key 非空占比>0",
       int(psql("SELECT count(*) FROM categories WHERE key IS NOT NULL")) > 0)

    # H3：site_name 不被清空（预置包 brand 全空）
    site_name1 = psql(
        "SELECT COALESCE((SELECT value FROM site_settings "
        "WHERE name='site_name'),'')"
    )
    ok("H3 site_name 不被空 brand 清空",
       site_name1 == site_name0 and site_name1 != "",
       (site_name0, site_name1))

    # H4：覆盖位键保持手工值（apply 跳过）
    cur_val1 = psql(
        "SELECT value FROM site_settings WHERE name='module_%s'" % probe_mod
    )
    ok("H4 覆盖位模块键不被 apply 复位", cur_val1 == cur_val, (cur_val, cur_val1))

    # H5：台账 changes 含分类段
    apply_id = r.get("data", {}).get("apply_id")
    ch = psql("SELECT changes FROM site_pack_applies WHERE id=%s" % apply_id)
    ok("H5 台账 changes 含 category_new",
       "category_new" in ch or "category_inactive" in ch)

    # ---- 还原：回滚这次 apply ----
    s, r = call("POST", "/admin/site-type-packs/applies/%s/rollback" % apply_id,
                None, token=token)
    ok("rollback 200", s == 200 and r.get("code") == 0, r)
    site_type2 = psql("SELECT value FROM site_settings WHERE name='site_type'")
    ok("回滚后 site_type 还原", site_type2 == site_type0,
       (site_type0, site_type2))
    name2 = psql(
        "SELECT COALESCE((SELECT value FROM site_settings "
        "WHERE name='site_name'),'')"
    )
    ok("回滚后 site_name 还原", name2 == site_name0, (site_name0, name2))
    # 清理探针覆盖位（不留测试痕迹）
    psql("DELETE FROM pack_module_overrides WHERE module_key='%s'" % probe_mod)

    print()
    print(f"# 还原核对：site_type={site_type2} == {site_type0}")


if __name__ == "__main__":
    main()
    summary()

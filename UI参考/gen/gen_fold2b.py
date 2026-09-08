# -*- coding: utf-8 -*-
"""双折叠第二批: 我的数据子菜单/论坛详情/课本管理/勋章分类/排行子类/游戏/工具/浏览状态/详情页签/站务 等 ~150页"""
import sys
sys.path.insert(0, '/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/gen')
from data import *

n = [250]
def next_id():
    n[0] += 1
    return f"e{n[0]}"

def f2_shell(title, content, label):
    aid = next_id()
    return f'''    <dc-artboard id="{aid}-f2p" label="{label}" width="720" height="900">
      <div class="fold2-screen">
        <div class="f2-statusbar"><span>9:41</span><span>📶 🔋 100%</span></div>
        <div class="f2-topbar"><div class="f2-logo"><img src="{OWL}" alt=""><b>{title}</b></div><div class="f2-search">🔍 搜索…</div><div class="f2-actions"><img class="f2-avatar" src="{OWL}" alt=""></div></div>
        <div class="f2-content" style="padding:14px">{content}</div>
        <div class="f2-dock"><span>🏠</span><span>🔍</span><span>📥</span><span>💬</span><span>👤</span></div>
      </div>
    </dc-artboard>
'''

def card(icon, bg, name, sub, right=''):
    return f'<div style="display:flex;gap:9px;align-items:center;background:#fff;border-radius:11px;padding:11px 12px"><span style="width:34px;height:34px;border-radius:9px;background:{bg};color:#fff;display:flex;align-items:center;justify-content:center;font-size:13px;flex-shrink:0">{icon}</span><div style="flex:1;min-width:0"><b style="font-size:12px;display:block;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{name}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">{sub}</p></div>{f"<div style=text-align:right;flex-shrink:0>{right}</div>" if right else ""}</div>'

def chip(t, on=False):
    return f'<span style="background:{"#2FA8FF" if on else "#F0F4F8"};color:{"#fff" if on else "#667"};border-radius:14px;padding:5px 12px;font-size:11px">{t}</span>'

def tag(t, c="#2FA8FF", bg="#E8F4FF"):
    return f'<span style="background:{bg};color:{c};border-radius:8px;padding:1px 6px;font-size:9px">{t}</span>'

out = []
L = out.append

def info_page(title, label, items, icon_bg="#E8F4FF"):
    L(f2_shell(title, f'<div style="display:grid;gap:9px">{"".join(card(ic, icon_bg, a, b) for ic,a,b in items)}</div>', label))

# ===== 1. 我的数据 11 个子菜单页 =====
L(f2_shell("绩效考核界面", '''
          <div style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);border-radius:14px;padding:16px;color:#fff;margin-bottom:10px"><p style="font-size:11px;opacity:.9;margin:0">保种员 5T 版 · 本月</p><b style="font-size:22px">200,000</b><p style="font-size:10px;opacity:.9;margin:3px 0 0">火花 · 考核期 2026-09-01 ~ 09-30</p></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:10px"><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><b style="font-size:18px;color:#2FA8FF">28/30</b><p style="color:#99a;font-size:10px;margin:2px 0 0">操作总数（要求30/最低10）</p></div><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><b style="font-size:18px;color:#2FBF9B">25/30</b><p style="color:#99a;font-size:10px;margin:2px 0 0">通过审核（要求30/最低10）</p></div></div>
          <div style="background:#fff;border-radius:11px;padding:12px"><b style="font-size:12px;color:#8895aa">📜 领取历史</b><div style="display:grid;gap:6px;margin-top:8px;font-size:11px"><div style="display:flex;justify-content:space-between"><span>gntv</span><b style="color:#2FBF9B">200,000</b></div><div style="display:flex;justify-content:space-between"><span>冷冷的风</span><b style="color:#2FBF9B">200,000</b></div><div style="display:flex;justify-content:space-between"><span>alan5914</span><b style="color:#2FBF9B">200,000</b></div></div></div>
''', "E251 · 双折叠 绩效考核"))
L(f2_shell("装饰品管理中心", f'''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:10px"><div style="background:linear-gradient(135deg,#FF8FC7,#FFC93C);border-radius:12px;padding:14px;color:#fff;text-align:center"><span style="font-size:26px">🖼️</span><b style="display:block;font-size:13px;margin-top:4px">星光头像框</b><p style="font-size:10px;opacity:.9;margin:2px 0 0">已装备</p></div><div style="background:#fff;border-radius:12px;padding:14px;text-align:center"><span style="font-size:26px">🌈</span><b style="display:block;font-size:13px;margin-top:4px">ID彩虹特效</b><p style="color:#99a;font-size:10px;margin:2px 0 0">未装备</p></div></div>
          <div style="display:grid;gap:8px">{"".join(card(ic, "#FFE8F5", nm, ds) for ic,nm,ds in [("✨","昵称发光特效","10,000 火花 · 未购买"),("🎀","学习风背景","3,000 火花 · 未购买"),("🎈","气泡装扮","2,500 火花 · 未购买"),("💎","稀有边框-银河","500,000 火花 · 限定")])}</div>
''', "E252 · 双折叠 装饰品中心"))
info_page("我的待审核", "E253 · 双折叠 待审核", [("⏳","识典古籍 六書正譌","种子 · 审核中 · 提交09-06"),("⏳","小学英语听力合集","种子 · 审核中 · 提交09-05"),("⏳","初中物理实验视频","种子 · 待补充 · 提交09-03"),("✅","高中数学讲义 PDF","已通过 · 09-01")], "#FFF4E0")
info_page("禁止中", "E254 · 双折叠 禁止中", [("🚫","暂无被禁种子","一切正常，继续保持！"),("✅","历史记录","无违规记录"),("💡","注意","请遵守分享率规范，避免作弊")], "#FFE8E8")
info_page("管理组", "E255 · 双折叠 管理组", [("🎧","一线客服","10 人 · 可申请加入"),("💬","批评家","5 人 · 可申请加入"),("🛡️","论坛版主","8 人 · 可申请加入"),("⚙️","常规管理员","6 人 · 可申请加入"),("👑","VIP","45 人 · 可申请加入")], "#E8F4FF")
info_page("PM 管理组", "E256 · 双折叠 PM管理组", [("📨","发给管理组","选择管理组成员"),("📤","已发送 PM","3 条 · 均已回复"),("⏳","待回复","1 条"),("🔔","通知设置","管理组消息即时提醒")], "#E8F0FF")
info_page("排行榜入口", "E257 · 双折叠 排行榜入口", [("📊","用户排行","上传/下载/分享率"),("🌐","社区排行","论坛活跃/在线时长"),("🧲","种子排行","Torrents/Seeding Size"),("⚡","速度排行","最快上传/下载"),("🎖️","勋章排行","收藏数/成就点")], "#E8F4FF")
info_page("站点日志", "E258 · 双折叠 站点日志", [("📋","2026-09-06","发布 23 个新种子"),("📋","2026-09-05","更新种子审核规范"),("📋","2026-09-03","修复下载计数异常"),("📋","2026-09-01","站免池达成 50%")], "#E8F8F2")
info_page("守护神", "E259 · 双折叠 守护神", [("🛡️","守护神计划","长期保种 10T 以上"),("🏆","当前排名","第 12 名"),("📊","我的保种量","12.4 TB"),("🎁","月度奖励","20,000 火花")], "#E8F0FF")
info_page("发送消息", "E260 · 双折叠 发送", [("📨","收件人","gntv"),("📝","主题","谢谢你的资源！"),("💬","内容预览","资源很好用，孩子很喜欢…"),("📤","发送记录","最近发送 09-06")], "#E8F4FF")
info_page("退出确认", "E261 · 双折叠 退出", [("👋","确定要退出登录吗？","下次登录需要重新验证"),("🔄","退出后","将回到登录页面"),("💡","提示","Cookie 将失效，需重新登录")], "#FFE8E8")

# ===== 2. 论坛：版块列表×4 + 帖子详情×3 + 发帖 =====
for fi, (fn, subs, c) in enumerate(FORUMS):
    L(f2_shell(fn, f'''
          <div style="display:flex;gap:6px;margin-bottom:10px"><span style="background:#E8F4FF;color:#2FA8FF;border-radius:12px;padding:4px 11px;font-size:10px">全部</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:4px 11px;font-size:10px">精华</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:4px 11px;font-size:10px">最新</span></div>
          <div style="display:grid;gap:8px">{"".join(f'<div style="background:#fff;border-radius:11px;padding:11px 12px"><div style="display:flex;justify-content:space-between;font-size:12px"><b>{s}</b><span style="color:#99a;font-size:10px">{cnt}</span></div><p style="color:#99a;font-size:10px;margin:4px 0 0">最新：{"IYUU辅种教程" if fi==0 else "新人报道" if fi==1 else "发邀贴" if fi==2 else "脚本分享"} · 09-06</p></div>' for s,cnt in subs)}</div>
''', f"E26{2+fi} · 双折叠 论坛-{fn.replace('📢 ','').replace('📚 ','').replace('🎁 ','').replace('💻 ','')}"))
for pi, (pt, pa, pc, pl) in enumerate([
    ("IYUU辅种保姆级教程", "gntv · 发布员", "23 回复 · 356 阅读", "一步一步教你配置 IYUU，自动辅种全站，新手也能轻松上手！"),
    ("新人必读：站点规则", "管理组 · 版主", "45 回复 · 892 阅读", "欢迎来到好学PT！请先阅读站点规则，避免违规被处罚。"),
    ("求推荐高中化学资料", "study_mom · User", "8 回复 · 45 阅读", "孩子上高中了，想找一些优质的化学学习资源，求推荐！"),
]):
    L(f2_shell("帖子详情", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:14px">{pt}</b><p style="color:#99a;font-size:10px;margin:5px 0 0">{pa} · {pl}</p></div>
          <div style="background:#fff;border-radius:12px;padding:12px;font-size:12px;color:#667;line-height:1.8;margin-bottom:10px">{pc}</div>
          <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:8px"><b style="font-size:12px">💬 全部回复</b><span style="color:#99a;font-size:10px">最新优先</span></div>
          <div style="display:grid;gap:8px"><div style="background:#fff;border-radius:11px;padding:11px"><div style="display:flex;gap:8px;align-items:center"><img src="{OWL}" style="width:26px;height:26px;border-radius:50%"><b style="font-size:11px">alan5914</b><span style="color:#99a;font-size:9px">09-06</span></div><p style="font-size:11px;color:#667;margin:6px 0 0">收藏了，感谢分享！</p></div><div style="background:#fff;border-radius:11px;padding:11px"><div style="display:flex;gap:8px;align-items:center"><img src="{OWL}" style="width:26px;height:26px;border-radius:50%"><b style="font-size:11px">kiririn</b><span style="color:#99a;font-size:9px">09-06</span></div><p style="font-size:11px;color:#667;margin:6px 0 0">写得很详细，赞！</p></div></div>
''', f"E26{6+pi} · 双折叠 帖子详情-{pt[:6]}"))
L(f2_shell("发帖", '''
          <div style="background:#fff;border-radius:12px;padding:14px;display:grid;gap:10px"><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">版块 *</label><select class="gi" style="width:100%;height:38px;font-size:12px"><option>学习交流俱乐部 · 小学部交流</option><option>本站事务区</option><option>发邀专区</option><option>技术交流</option></select></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">标题 *</label><input class="gi" placeholder="请输入帖子标题" style="width:100%;height:38px;font-size:12px"></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">内容 *</label><textarea class="gi" placeholder="请输入帖子内容…" style="width:100%;height:120px;font-size:12px;resize:none"></textarea></div><div style="display:flex;gap:6px;flex-wrap:wrap"><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 11px;font-size:10px">#求助</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 11px;font-size:10px">#分享</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 11px;font-size:10px">#讨论</span></div></div>
          <button class="f2-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px;margin-top:10px">📤 发布帖子</button>
''', "E269 · 双折叠 发帖"))

# ===== 3. 课本管理 =====
info_page("我的课本提交", "E270 · 双折叠 我的提交", [("📘","人教版语文四年级上册","已通过 · 09-06"),("📗","人教版数学五年级","审核中 · 09-05"),("📙","部编版语文三年级","已通过 · 08-30"),("➕","提交新课本","选择科目与版本")], "#E8F8F2")
info_page("课本管理后台", "E271 · 双折叠 管理课本", [("⚙️","科目管理","29 个科目"),("📚","课本管理","1,234 本课本"),("⏳","待审核课本","5 本"),("📊","下载统计","本月 45,892 次")], "#E8F0FF")
L(f2_shell("创建课本", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;display:grid;gap:10px"><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">科目 *</label><select class="gi" style="width:100%;height:38px;font-size:12px">{"".join(f"<option>{s}</option>" for s in SUBJECTS[:8])}</select></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">年级 *</label><div style="display:flex;gap:6px;flex-wrap:wrap">{"".join(chip(x, on=(i==0)) for i,x in enumerate(["一年级","二年级","三年级","四年级","五年级","六年级","初一","初二"]))}</div></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">版本 *</label><select class="gi" style="width:100%;height:38px;font-size:12px"><option>人教版</option><option>部编版</option><option>北师大版</option><option>苏教版</option></select></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">简介</label><textarea class="gi" placeholder="课本说明…" style="width:100%;height:64px;font-size:12px;resize:none"></textarea></div></div>
          <button class="f2-btn" style="background:#2FBF9B;color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px;margin-top:10px">➕ 创建课本</button>
''', "E272 · 双折叠 创建课本"))
for gi, (gn, subs) in enumerate([("小学", SUBJECTS[:6]), ("初中", SUBJECTS[4:10]), ("高中", SUBJECTS[8:14])]):
    L(f2_shell(f"课本-{gn}", f'''
          <div style="display:grid;gap:8px">{"".join(f'<div style="background:#fff;border-radius:11px;padding:11px 12px;display:flex;justify-content:space-between;align-items:center"><div style="display:flex;gap:9px;align-items:center"><span style="font-size:20px">📘</span><div><b style="font-size:12px">{s}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">人教版 · 12本 · 356次下载</p></div></div><span style="color:#2FA8FF;font-size:11px">查看 ›</span></div>' for s in subs)}</div>
''', f"E27{3+gi} · 双折叠 课本-{gn}"))

# ===== 4. 勋章分类 ×6 =====
medal_cats = [
    ("二十四节气", [("🌸","立春","学习课堂"),("🌧️","雨水","学习课堂"),("⚡","惊蛰","学习课堂"),("🌿","春分","学习课堂"),("🔥","立夏","学习课堂")]),
    ("学习课堂", [("📚","初出茅庐","学习课堂"),("✏️","勤学苦练","学习课堂"),("🏆","学霸之路","学习课堂"),("🎓","金榜题名","学习课堂")]),
    ("节日系列", [("🎆","开站纪念","开站系列"),("🏮","春节限定","节日系列"),("🥮","中秋限定","节日系列"),("🎃","万圣节","节日系列")]),
    ("开站系列", [("🌟","首批用户","开站系列"),("🚀","开站元勋","开站系列"),("💎","创始会员","开站系列")]),
    ("工作组", [("🛡️","维护先锋","工作组"),("🎧","客服之星","工作组"),("🖥️","技术大牛","工作组")]),
    ("未分类", [("🎲","幸运儿","未分类"),("🎯","签到狂魔","未分类"),("🦉","猫头鹰之友","未分类")]),
]
for mi, (cn, items) in enumerate(medal_cats):
    L(f2_shell(f"勋章-{cn}", f'''
          <div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:10px"><b style="font-size:13px">{cn}</b><p style="color:#99a;font-size:11px;margin:2px 0 0">已收集 {len(items)//2} / {len(items)} 枚</p></div>
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px">{"".join(f'<div style="background:#fff;border-radius:11px;padding:11px;text-align:center;{"border:2px solid #E8F4FF" if i%2==0 else "opacity:.4"}"><span style="font-size:22px">{m}</span><b style="display:block;font-size:10px;margin-top:3px">{nm}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">{ds}</p></div>' for i,(m,nm,ds) in enumerate(items))}</div>
''', f"E27{6+mi} · 双折叠 勋章-{cn}"))

# ===== 5. 排行榜子分类 ×8 =====
rank_subs = [
    ("上传者排行", "上传量", ["dgvge","109.697 TB"], ["Rick","101.492 TB"], ["kiririn","81.859 TB"]),
    ("下载者排行", "下载量", ["shmt86","56.2 TB"], ["JaCksonPp","42.8 TB"], ["gntv","2.83 TB"]),
    ("做种数排行", "做种数", ["gntv","16,348"], ["alan5914","1,095"], ["ranfish","365"]),
    ("保种大小排行", "保种大小", ["dgvge","85.4 TB"], ["Rick","76.2 TB"], ["kiririn","61.9 TB"]),
    ("最快上传者", "速度", ["dgvge","38.2 MB/s"], ["Rick","29.6 MB/s"], ["shmt86","24.1 MB/s"]),
    ("最快下载者", "速度", ["JaCksonPp","42.5 MB/s"], ["shmt86","35.8 MB/s"], ["kiririn","22.9 MB/s"]),
    ("最佳分享者", "分享率", ["dgvge","99.9"], ["Rick","88.4"], ["gntv","38.5"]),
    ("最差分享者", "分享率", ["user_a","0.12"], ["user_b","0.21"], ["user_c","0.28"]),
]
for ri, (rn, col, r1, r2, r3) in enumerate(rank_subs):
    rows = "".join(f'<div style="display:flex;align-items:center;gap:9px;padding:10px 12px;background:{"linear-gradient(135deg,#FFF8E8,#FFF4D6)" if i==0 else "#fff"};border-radius:11px"><span style="width:20px;text-align:center;font-weight:700;color:{"#FFC93C" if i<3 else "#99a"}">{i+1}</span><img src="{OWL}" style="width:30px;height:30px;border-radius:50%"><div style="flex:1"><b style="font-size:12px">{u}</b><p style="color:#99a;font-size:9px;margin:0">{cl}</p></div><b style="font-size:12px">{v}</b></div>' for i,(u,cl,v) in enumerate([(r1[0], rn, r1[1]), (r2[0], rn, r2[1]), (r3[0], rn, r3[1])]))
    L(f2_shell("排行榜", f'''
          <div style="display:flex;gap:6px;margin-bottom:10px;flex-wrap:wrap">{"".join(chip(x, on=(x==rn)) for x in ["上传者","下载者","做种","保种大小","最快上传","最快下载","最佳分享","最差分享"])}</div>
          <div style="display:grid;gap:8px">{rows}</div>
          <div style="display:flex;gap:6px;margin-top:10px;font-size:10px"><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:4px 10px">范围 Top 21</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:4px 10px">更新 09-07</span></div>
''', f"E28{2+ri} · 双折叠 排行-{rn}"))

# ===== 6. 娱乐市场游戏 ×5 =====
L(f2_shell("五子棋", '''
          <div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:10px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:14px">⚫ 五子棋</b><p style="color:#99a;font-size:10px;margin:2px 0 0">18 胜 12 负 · 积分 1,260</p></div><span style="background:#2FA8FF;color:#fff;border-radius:10px;padding:6px 14px;font-size:11px">匹配对手</span></div>
          <div style="background:#F5E8C8;border-radius:8px;padding:14px;margin-bottom:10px"><div style="display:grid;grid-template-columns:repeat(5,1fr);gap:0;max-width:300px;margin:0 auto">'''.replace("grid-template-columns:repeat(5,1fr);gap:0;max-width:300px;margin:0 auto", "grid-template-columns:repeat(5,1fr);gap:0;max-width:300px;margin:0 auto") + '''<div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A;border-radius:50%;box-shadow:inset 0 0 0 3px #2FA8FF"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A;border-radius:50%"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A;border-radius:50%;box-shadow:inset 0 0 0 3px #1F2A44"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div></div></div>
          <div style="display:flex;justify-content:space-between;font-size:11px;color:#99a"><span>你执黑 · 回合 12</span><span>对手思考中…</span></div>
''', "E290 · 双折叠 五子棋"))
L(f2_shell("刮刮乐", '''
          <div style="background:linear-gradient(135deg,#FF7A59,#FFC93C);border-radius:14px;padding:16px;color:#fff;margin-bottom:10px;text-align:center"><b style="font-size:16px">🎫 刮刮乐</b><p style="font-size:11px;opacity:.9;margin:4px 0 0">2,000 火花 / 次 · 最高赢 50,000</p></div>
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin-bottom:10px">'''.replace("grid-template-columns:repeat(3,1fr);gap:8px;margin-bottom:10px","grid-template-columns:repeat(3,1fr);gap:8px;margin-bottom:10px") + '''<div style="background:linear-gradient(135deg,#B8C4D9,#8B9BB8);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px;color:#fff">🎁</div><div style="background:linear-gradient(135deg,#B8C4D9,#8B9BB8);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px;color:#fff">🎁</div><div style="background:linear-gradient(135deg,#B8C4D9,#8B9BB8);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px;color:#fff">🎁</div><div style="background:linear-gradient(135deg,#FFD54F,#FFB300);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px;color:#fff">🎉</div><div style="background:linear-gradient(135deg,#B8C4D9,#8B9BB8);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px;color:#fff">🎁</div><div style="background:linear-gradient(135deg,#B8C4D9,#8B9BB8);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px;color:#fff">🎁</div></div>
          <div style="background:#FFF8E8;border-radius:10px;padding:10px;font-size:11px;color:#667;margin-bottom:10px">🎉 刮出「好运」！获得 2,000 火花</div>
          <button class="f2-btn" style="background:#FF7A59;color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px">🎫 再刮一次（2,000 火花）</button>
''', "E291 · 双折叠 刮刮乐"))
L(f2_shell("九宫格抽奖", '''
          <div style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);border-radius:14px;padding:16px;color:#fff;margin-bottom:10px;text-align:center"><b style="font-size:16px">🎯 九宫格抽奖</b><p style="font-size:11px;opacity:.9;margin:4px 0 0">1,000 火花 / 次 · 大奖 100,000</p></div>
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:6px;max-width:320px;margin:0 auto 10px">'''.replace("grid-template-columns:repeat(3,1fr);gap:6px;max-width:320px;margin:0 auto 10px","grid-template-columns:repeat(3,1fr);gap:6px;max-width:320px;margin:0 auto 10px") + '''<div style="background:#fff;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px">📚</div><div style="background:#fff;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px">💰</div><div style="background:#fff;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px">🎖️</div><div style="background:#fff;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px">🎁</div><div style="background:#2FA8FF;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px;color:#fff;box-shadow:0 0 16px rgba(47,168,255,.6)">🎯</div><div style="background:#fff;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px">⭐</div><div style="background:#fff;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px">💎</div><div style="background:#fff;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px">🔥</div><div style="background:#fff;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px">🌈</div></div>
          <button class="f2-btn" style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px">🎯 开始抽奖</button>
''', "E292 · 双折叠 九宫格"))
L(f2_shell("猜大小", '''
          <div style="background:linear-gradient(135deg,#2FBF9B,#2FA8FF);border-radius:14px;padding:16px;color:#fff;margin-bottom:10px;text-align:center"><b style="font-size:16px">🎲 猜大小</b><p style="font-size:11px;opacity:.9;margin:4px 0 0">押注 500~50,000 火花 · 1.95 倍</p></div>
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px;text-align:center"><p style="color:#99a;font-size:10px;margin:0">本期点数</p><b style="font-size:34px">12</b><div style="display:flex;gap:6px;justify-content:center;margin-top:6px"><span style="font-size:22px">⚀</span><span style="font-size:22px">⚂</span><span style="font-size:22px">⚄</span></div><p style="font-size:11px;margin:6px 0 0;color:#FF7A59">大！</p></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:8px"><div style="background:#E8F4FF;border-radius:11px;padding:12px;text-align:center"><b style="font-size:15px;color:#2FA8FF">押 大</b><p style="color:#99a;font-size:10px;margin:2px 0 0">1.95x</p></div><div style="background:#FFE8F5;border-radius:11px;padding:12px;text-align:center"><b style="font-size:15px;color:#FF8FC7">押 小</b><p style="color:#99a;font-size:10px;margin:2px 0 0">1.95x</p></div></div>
          <div style="background:#fff;border-radius:10px;padding:10px;font-size:11px;color:#99a">🎉 上局结果：押大 +1,950 火花</div>
''', "E293 · 双折叠 猜大小"))
L(f2_shell("卡牌合成", '''
          <div style="background:linear-gradient(135deg,#8B5CF6,#5B6BF5);border-radius:14px;padding:16px;color:#fff;margin-bottom:10px;text-align:center"><b style="font-size:16px">🃏 卡牌合成</b><p style="font-size:11px;opacity:.9;margin:4px 0 0">3 张同阶合成 1 张高阶</p></div>
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin-bottom:10px"><div style="background:linear-gradient(135deg,#E8F4FF,#D6E9FF);border-radius:12px;padding:14px;text-align:center"><span style="font-size:30px">🦉</span><b style="display:block;font-size:11px;margin-top:3px">猫头鹰·N</b></div><div style="background:linear-gradient(135deg,#E8F4FF,#D6E9FF);border-radius:12px;padding:14px;text-align:center"><span style="font-size:30px">🦉</span><b style="display:block;font-size:11px;margin-top:3px">猫头鹰·N</b></div><div style="background:linear-gradient(135deg,#E8F4FF,#D6E9FF);border-radius:12px;padding:14px;text-align:center"><span style="font-size:30px">🦉</span><b style="display:block;font-size:11px;margin-top:3px">猫头鹰·N</b></div></div>
          <div style="background:#fff;border-radius:11px;padding:12px;margin-bottom:10px;text-align:center;border:2px dashed #8B5CF6"><span style="font-size:16px;color:#8B5CF6">＋ 合成 →</span><div style="font-size:24px;margin-top:4px">📘</div><b style="font-size:12px;display:block;margin-top:2px">古籍·R</b></div>
          <div style="background:#fff;border-radius:10px;padding:10px;font-size:11px;color:#99a">📦 我的卡牌：N×8 · R×3 · SR×1 · 碎片 18/30</div>
''', "E294 · 双折叠 卡牌合成"))

# ===== 7. 工具箱 ×6 =====
tool_pages = [
    ("获取图床Token", [("🔑","Token 已获取","有效期 30 天"),("📋","Token","hxpt_img_8f3a9c…"),("💡","用途","用于官方图床 API 上传"),("🔄","刷新 Token","点击重新生成")], "#E8F0FF"),
    ("官方图床", [("🖼️","上传图片","支持 jpg/png/webp ≤10MB"),("📤","最近上传","6 张 · 今天"),("🔗","外链格式","https://img.hxpt.org/…"),("💡","建议","发布资源时使用图床外链")], "#E8F4FF"),
    ("图床批量上传工具", [("📁","批量上传","一次最多 20 张"),("⏳","上传进度","8/20 张"),("✅","成功","8 张"),("❌","失败","0 张")], "#E8F8F2"),
    ("AutoFeed 转种", [("⚙️","AutoFeed","自动抓取转种工具"),("🔄","运行状态","已停止"),("📡","最近抓取","09-06 · 12 个种子"),("⚙️","配置","修改 RSS 源与规则")], "#E8F0FF"),
    ("IYUU 辅种", [("🔁","IYUU 辅种","跨站自动辅种"),("🔄","运行状态","已开启"),("🎯","辅种数量","1,234 个"),("📊","今日收益","+8.2 GB 上传")], "#E8F4FF"),
    ("照片压缩工具", [("🖼️","压缩图片","支持 jpg/png ≤30MB"),("📉","压缩率","最大 80%"),("✅","最近处理","12 张 · 今天"),("💡","提示","发布资源前先压缩扫描页")], "#E8F8F2"),
]
for ti, (tn, items, bg) in enumerate(tool_pages):
    info_page(tn, f"E29{5+ti} · 双折叠 {tn}", items, bg)

# ===== 8. 浏览状态 ×4 =====
L(f2_shell("浏览-排序", f'''
          <div style="display:flex;gap:6px;margin-bottom:10px;flex-wrap:wrap">{"".join(chip(x, on=(i==0)) for i,x in enumerate(["最新","热门","大小","做种数","发布时间"]))}</div>
          <div style="display:grid;gap:8px">{"".join(card(t[1], t[2], t[0][:26]+"…" if len(t[0])>26 else t[0], f"{t[3]} · {t[4]} · {t[5]}做种") for t in TORRENTS[:5])}</div>
          <div style="padding:12px;text-align:center;color:#99a;font-size:11px">第 2 / 486 页</div>
''', "E301 · 双折叠 浏览-排序"))
L(f2_shell("浏览-筛选", f'''
          <div style="background:#fff;border-radius:11px;padding:12px;margin-bottom:10px;display:grid;gap:8px"><div><label style="font-size:10px;color:#99a">分类</label><div style="display:flex;gap:5px;flex-wrap:wrap;margin-top:4px">{"".join(chip(c, on=(c=="初中")) for c,_ in [("全部",""),("学前教育",""),("小学",""),("初中",""),("职高",""),("高中",""),("教育影音",""),("纪录片","")])}</div></div><div><label style="font-size:10px;color:#99a">标签</label><div style="display:flex;gap:5px;flex-wrap:wrap;margin-top:4px">{"".join(chip(x) for x in ["免费","保种","官方","2x","新种"])}</div></div><div><label style="font-size:10px;color:#99a">大小</label><div style="display:flex;gap:5px;flex-wrap:wrap;margin-top:4px">{"".join(chip(x) for x in ["<100MB","100-500MB","500MB-1G","1-5G",">5G"])}</div></div></div>
          <button class="f2-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;width:100%;padding:11px;border-radius:10px;font-size:13px">🔍 应用筛选</button>
          <div style="padding:10px;text-align:center;color:#99a;font-size:11px">共 3,421 个种子符合条件</div>
''', "E302 · 双折叠 浏览-筛选"))
L(f2_shell("浏览-分页", f'''
          <div style="display:grid;gap:8px;margin-bottom:10px">{"".join(card(t[1], t[2], t[0][:24]+"…" if len(t[0])>24 else t[0], f"{t[4]} · {t[5]}做种") for t in TORRENTS[:5])}</div>
          <div style="display:flex;justify-content:center;gap:6px;align-items:center;font-size:12px"><span style="background:#F0F4F8;color:#99a;border-radius:9px;padding:6px 11px">‹</span><span style="background:#2FA8FF;color:#fff;border-radius:9px;padding:6px 12px;font-weight:700">1</span><span style="background:#F0F4F8;color:#667;border-radius:9px;padding:6px 12px">2</span><span style="background:#F0F4F8;color:#667;border-radius:9px;padding:6px 12px">3</span><span style="background:#F0F4F8;color:#99a;border-radius:9px;padding:6px 12px">…</span><span style="background:#F0F4F8;color:#667;border-radius:9px;padding:6px 12px">486</span><span style="background:#F0F4F8;color:#99a;border-radius:9px;padding:6px 11px">›</span></div>
''', "E303 · 双折叠 浏览-分页"))
L(f2_shell("浏览-列表模式", f'''
          <div style="display:flex;gap:6px;margin-bottom:10px"><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 11px;font-size:10px">☰ 列表</span><span style="background:#E8F4FF;color:#2FA8FF;border-radius:12px;padding:5px 11px;font-size:10px">▦ 网格</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 11px;font-size:10px">🖼️ 大图</span></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px">{"".join(f'<div style="background:#fff;border-radius:11px;padding:10px;text-align:center"><span style="font-size:24px">{t[1]}</span><b style="display:block;font-size:11px;margin-top:4px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{t[0][:16]}</b><p style="color:#99a;font-size:9px;margin:2px 0 0">{t[4]} · {t[5]}做种</p></div>' for t in TORRENTS[:8])}</div>
''', "E304 · 双折叠 浏览-网格"))

# ===== 9. 详情页签 ×5 =====
L(f2_shell("详情-简介", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><div style="display:flex;gap:8px;align-items:flex-start"><b style="font-size:14px;flex:1">{TORRENTS[0][0]}</b><span class="tag tag-free">Free</span></div><p style="color:#99a;font-size:10px;margin:5px 0 0">发布者 gntv · 2026-09-06</p></div>
          <div style="display:flex;gap:5px;margin-bottom:10px;font-size:10px"><span style="background:#E8F4FF;color:#2FA8FF;border-radius:12px;padding:5px 12px;font-weight:600">📝 简介</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 12px">📁 文件</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 12px">🔄 做种</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 12px">📥 下载</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 12px">💬 评论</span></div>
          <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:12px;color:#8895aa">资源简介</b><p style="font-size:12px;color:#667;line-height:2;margin:8px 0 0">识典古籍系列收录中华经典古籍的简繁双版扫描资源，含 TXT、PDF、EPUB 格式及 WEBP 扫描页，适合古文学习与研究使用。本站官方制作组 HX 出品。</p><div style="background:#F8FAFC;border-radius:9px;padding:10px;margin-top:10px;font-size:11px;color:#667"><b>📋 制作说明</b><br>扫描页 44 张 · OCR 校对 · 简繁对照</div></div>
''', "E305 · 双折叠 详情-简介"))
L(f2_shell("详情-文件", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:13px">{TORRENTS[0][0][:24]}…</b><p style="color:#99a;font-size:10px;margin:3px 0 0">共 48 个文件 · 384 MB</p></div>
          <div style="display:flex;gap:5px;margin-bottom:10px;font-size:10px"><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 12px">📝 简介</span><span style="background:#E8F4FF;color:#2FA8FF;border-radius:12px;padding:5px 12px;font-weight:600">📁 文件</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 12px">🔄 做种</span></div>
          <div style="background:#fff;border-radius:12px;padding:12px;display:grid;gap:5px;font-size:11px">'''.replace("display:grid;gap:5px;font-size:11px","display:grid;gap:5px;font-size:11px") + '''<div style="display:flex;justify-content:space-between;padding:7px 8px;border-bottom:1px solid #F0F4F8"><span>📄 六書正譌.txt</span><span style="color:#99a">128MB</span></div><div style="display:flex;justify-content:space-between;padding:7px 8px;border-bottom:1px solid #F0F4F8"><span>📄 六書正譌.pdf</span><span style="color:#99a">96MB</span></div><div style="display:flex;justify-content:space-between;padding:7px 8px;border-bottom:1px solid #F0F4F8"><span>📄 六書正譌.epub</span><span style="color:#99a">12MB</span></div><div style="display:flex;justify-content:space-between;padding:7px 8px;border-bottom:1px solid #F0F4F8"><span>📁 扫描页/</span><span style="color:#99a">44文件 · 148MB</span></div></div>
''', "E306 · 双折叠 详情-文件"))
L(f2_shell("详情-做种列表", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:13px">🔄 做种列表</b><p style="color:#99a;font-size:10px;margin:3px 0 0">12 位同学正在做种 · 合计 2.1 MB/s 上传</p></div>
          <div style="display:grid;gap:8px">'''.replace("display:grid;gap:8px","display:grid;gap:8px") + '''<div style="background:#fff;border-radius:11px;padding:10px 12px;display:flex;justify-content:space-between;font-size:11px"><div style="display:flex;gap:8px;align-items:center"><img src="''' + OWL + '''" style="width:26px;height:26px;border-radius:50%"><b>gntv</b><span style="color:#99a">发布员</span></div><div style="text-align:right"><b style="color:#2FBF9B">1.2 MB/s</b><p style="color:#99a;font-size:9px;margin:0">上传中</p></div></div><div style="background:#fff;border-radius:11px;padding:10px 12px;display:flex;justify-content:space-between;font-size:11px"><div style="display:flex;gap:8px;align-items:center"><img src="''' + OWL + '''" style="width:26px;height:26px;border-radius:50%"><b>alan5914</b><span style="color:#99a">上传者</span></div><div style="text-align:right"><b style="color:#2FBF9B">0.8 MB/s</b><p style="color:#99a;font-size:9px;margin:0">上传中</p></div></div><div style="background:#fff;border-radius:11px;padding:10px 12px;display:flex;justify-content:space-between;font-size:11px"><div style="display:flex;gap:8px;align-items:center"><img src="''' + OWL + '''" style="width:26px;height:26px;border-radius:50%"><b>study_mom</b><span style="color:#99a">User</span></div><div style="text-align:right"><b style="color:#99a">0.1 MB/s</b><p style="color:#99a;font-size:9px;margin:0">上传中</p></div></div></div>
''', "E307 · 双折叠 详情-做种"))
L(f2_shell("详情-下载列表", '''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:13px">📥 下载列表</b><p style="color:#99a;font-size:10px;margin:3px 0 0">3 位同学正在下载</p></div>
          <div style="display:grid;gap:8px"><div style="background:#fff;border-radius:11px;padding:10px 12px;display:flex;justify-content:space-between;font-size:11px"><div style="display:flex;gap:8px;align-items:center"><img src="''' + OWL + '''" style="width:26px;height:26px;border-radius:50%"><b>kiririn</b></div><div style="text-align:right"><b style="color:#2FA8FF">1.8 MB/s</b><p style="color:#99a;font-size:9px;margin:0">下载中 · 65%</p></div></div><div style="background:#fff;border-radius:11px;padding:10px 12px;display:flex;justify-content:space-between;font-size:11px"><div style="display:flex;gap:8px;align-items:center"><img src="''' + OWL + '''" style="width:26px;height:26px;border-radius:50%"><b>shmt86</b></div><div style="text-align:right"><b style="color:#2FA8FF">0.9 MB/s</b><p style="color:#99a;font-size:9px;margin:0">下载中 · 32%</p></div></div><div style="background:#fff;border-radius:11px;padding:10px 12px;display:flex;justify-content:space-between;font-size:11px"><div style="display:flex;gap:8px;align-items:center"><img src="''' + OWL + '''" style="width:26px;height:26px;border-radius:50%"><b>JaCksonPp</b></div><div style="text-align:right"><b style="color:#99a">等待</b><p style="color:#99a;font-size:9px;margin:0">排队中</p></div></div></div>
''', "E308 · 双折叠 详情-下载"))
L(f2_shell("详情-评论", '''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:13px">💬 评论（12）</b><p style="color:#99a;font-size:10px;margin:3px 0 0">4.9 分 · 多数好评</p></div>
          <div style="display:grid;gap:8px;margin-bottom:10px"><div style="background:#fff;border-radius:11px;padding:11px"><div style="display:flex;gap:8px;align-items:center"><img src="''' + OWL + '''" style="width:26px;height:26px;border-radius:50%"><b style="font-size:11px">study_mom</b><span style="color:#FFC93C;font-size:10px">★★★★★</span><span style="color:#99a;font-size:9px;margin-left:auto">09-06</span></div><p style="font-size:11px;color:#667;margin:6px 0 0">扫描页很清晰，孩子学习古文太方便了！</p></div><div style="background:#fff;border-radius:11px;padding:11px"><div style="display:flex;gap:8px;align-items:center"><img src="''' + OWL + '''" style="width:26px;height:26px;border-radius:50%"><b style="font-size:11px">kiririn</b><span style="color:#FFC93C;font-size:10px">★★★★★</span><span style="color:#99a;font-size:9px;margin-left:auto">09-06</span></div><p style="font-size:11px;color:#667;margin:6px 0 0">EPUB 版本体验极佳，感谢制作组！</p></div></div>
          <div style="background:#fff;border-radius:11px;padding:11px;display:flex;gap:8px"><input class="gi" placeholder="写下你的评价…" style="flex:1;height:36px;font-size:12px"><span style="background:#2FA8FF;color:#fff;border-radius:9px;padding:0 14px;display:flex;align-items:center;font-size:12px">发送</span></div>
''', "E309 · 双折叠 详情-评论"))

# ===== 10. 请求/字幕/保种/官种详情 ×5 =====
L(f2_shell("求种详情", '''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:14px">🙋 小学数学思维训练 4年级</b><p style="color:#99a;font-size:10px;margin:5px 0 0">study_mom 发起 · 09-05 · 悬赏 5,000 火花</p></div>
          <div style="background:#fff;border-radius:12px;padding:12px;font-size:12px;color:#667;line-height:1.9;margin-bottom:10px">想要一套 4 年级的数学思维训练题集，最好是 PDF 扫描版，孩子暑假在家练习用。</div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:10px"><div style="background:#F0F7FF;border-radius:10px;padding:11px;text-align:center"><b style="font-size:18px;color:#2FA8FF">3</b><p style="color:#99a;font-size:10px;margin:2px 0 0">人已求</p></div><div style="background:#FFF8E8;border-radius:10px;padding:11px;text-align:center"><b style="font-size:18px;color:#FF7A59">5,000</b><p style="color:#99a;font-size:10px;margin:2px 0 0">悬赏火花</p></div></div>
          <button class="f2-btn" style="background:#FF7A59;color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px">💛 我也要求这个资源</button>
''', "E310 · 双折叠 求种详情"))
L(f2_shell("字幕详情", '''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:14px">🎬 蓝色星球 E01 中英双语</b><p style="color:#99a;font-size:10px;margin:5px 0 0">kiririn 上传 · 09-06 · ASS</p></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:10px"><div style="background:#fff;border-radius:11px;padding:11px;text-align:center"><b style="font-size:14px">2,304</b><p style="color:#99a;font-size:10px;margin:2px 0 0">下载次数</p></div><div style="background:#fff;border-radius:11px;padding:11px;text-align:center"><b style="font-size:14px">双语</b><p style="color:#99a;font-size:10px;margin:2px 0 0">语言</p></div></div>
          <button class="f2-btn" style="background:linear-gradient(135deg,#2FBF9B,#2FA8FF);color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px;margin-bottom:8px">⬇️ 下载字幕</button>
          <div style="background:#fff;border-radius:10px;padding:10px;font-size:11px;color:#99a">📊 关联种子：蓝色星球 第三季 · 豆瓣 9.8</div>
''', "E311 · 双折叠 字幕详情"))
info_page("保种区-官方保种", "E312 · 双折叠 保种-官方", [("🛡️","官方保种种子","1,234 个 · 全部做种中"),("🎁","免费下载","保种区种子全部免费"),("⏳","保种要求","做种 >7 天自动移出"),("🎉","免费延续","移出后免费状态延续 3 天")], "#E8F8F2")
info_page("保种区-全部保种", "E313 · 双折叠 保种-全部", [("🛡️","全部保种种子","856 个"),("📂","分类","教育/高中部/高职部/初中部/小学部"),("🎁","免费下载","所有保种种子免费"),("📊","我的保种","12.4 TB · 排名 12")], "#E8F8F2")
L(f2_shell("官种详情", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><div style="display:flex;gap:8px;align-items:flex-start"><b style="font-size:14px;flex:1">{OFFICIALS[0][0]}</b><span class="tag" style="background:#FFF4E0;color:#FF7A59">官种</span></div><p style="color:#99a;font-size:10px;margin:5px 0 0">HX 官方制作组 · 2026-09-06 · {OFFICIALS[0][1]}</p></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:10px"><div style="background:#FFF8E8;border-radius:10px;padding:11px;text-align:center"><b style="font-size:18px;color:#FF7A59">5x</b><p style="color:#99a;font-size:10px;margin:2px 0 0">官种火花加成</p></div><div style="background:#F0F7FF;border-radius:10px;padding:11px;text-align:center"><b style="font-size:18px;color:#2FA8FF">12</b><p style="color:#99a;font-size:10px;margin:2px 0 0">做种</p></div></div>
          <div style="background:#fff;border-radius:11px;padding:12px;font-size:11px;color:#667;line-height:1.9;margin-bottom:10px">官种由官方制作组 HX 发布，质量有保障。做种官种可获得 5 倍火花收益加成！</div>
          <button class="f2-btn" style="background:linear-gradient(135deg,#FF7A59,#FFC93C);color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px">⭐ 下载官种</button>
''', "E314 · 双折叠 官种详情"))

# ===== 11. 站务 ×4 =====
info_page("关于本站", "E315 · 双折叠 关于", [("🦉","好学PT HxPT","幼小初高学习资源 PT 站"),("🌱","愿景","好学者如春苗，日有所长"),("📅","开站","2025-07-16"),("🎯","定位","学前教育/小学/初中/职高/高中/教育影音/纪录片")], "#E8F4FF")
info_page("捐赠支持", "E316 · 双折叠 捐赠", [("💛","捐赠站免池","助力全站双倍免费"),("🎁","捐赠档位","1,000~1,000,000 火花"),("🏆","捐赠榜","gntv · dgvge · kiririn"),("💡","回馈","达 200 万下月全局免费")], "#FFF8E8")
info_page("友情链接", "E317 · 双折叠 友链", [("🔗","好学PT 官方","www.hxpt.org"),("🔗","识典古籍","shidian.com"),("🔗","学习资源导航","edu.hxpt.org"),("➕","申请友链","联系管理组")], "#E8F0FF")
info_page("联系我们", "E318 · 双折叠 联系我们", [("📧","邮箱","admin@hxpt.org"),("💬","PM","站内信联系管理组"),("🎧","客服","一线客服 10 人在线"),("⏰","响应","平均 24h 内回复")], "#E8F4FF")

# ===== 12. 更多分类浏览 ×4 =====
for ci, cn in enumerate(["学前教育","职高","教育影音","纪录片"]):
    L(f2_shell(f"浏览-{cn}", f'''
          <div style="background:linear-gradient(135deg,{["#FF8FC7","#8B5CF6","#2FBF9B","#FF7A59"][ci]},{["#FFC93C","#5B6BF5","#2FA8FF","#FFC93C"][ci]});border-radius:12px;padding:13px;color:#fff;margin-bottom:10px"><b style="font-size:15px">{cn}</b><p style="font-size:10px;opacity:.9;margin:3px 0 0">{["幼小衔接启蒙资源","职业技能学习资料","纪录片与教育影音","高品质纪录片合集"][ci]}</p></div>
          <div style="display:grid;gap:8px">{"".join(card(t[1], t[2], t[0][:26]+"…" if len(t[0])>26 else t[0], f"{t[3]} · {t[4]} · {t[5]}做种", tag(t[6].split("/")[0], bg="#E8F8F2" if t[6].startswith("Free") else "#FFF4E0")) for t in TORRENTS[ci*4:(ci+1)*4])}</div>
          <div style="padding:12px;text-align:center;color:#99a;font-size:11px">共 {[1820,1204,3402,2210][ci]} 个种子</div>
''', f"E31{9+ci} · 双折叠 浏览-{cn}"))

# ===== 13. 更多种子详情 ×6 =====
for ti, t in enumerate(TORRENTS[6:12]):
    L(f2_shell("种子详情", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><div style="display:flex;gap:8px;align-items:flex-start"><b style="font-size:14px;flex:1;line-height:1.5">{t[0]}</b><span class="tag tag-free">Free</span></div><p style="color:#99a;font-size:10px;margin:6px 0 0">发布者 gntv · 发布员 · 2026-09-06</p><div style="display:flex;gap:6px;margin-top:8px;flex-wrap:wrap">{"".join(tag(x) for x in t[6].split("/"))}</div></div>
          <div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:10px"><b style="font-size:12px;color:#8895aa">基本信息</b><div style="display:grid;gap:6px;margin-top:8px;font-size:12px"><span style="display:flex;justify-content:space-between"><span style="color:#99a">大小</span><b>{t[4]}</b></span><span style="display:flex;justify-content:space-between"><span style="color:#99a">类型</span><b>{t[3]}</b></span><span style="display:flex;justify-content:space-between"><span style="color:#99a">做种/下载</span><b>{t[5]} / 3</b></span></div></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:10px"><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:16px;color:#2FA8FF">{t[5]}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">做种</p></div><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:16px;color:#2FBF9B">3</b><p style="color:#99a;font-size:10px;margin:2px 0 0">下载</p></div></div>
          <button class="f2-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px;margin-bottom:8px">⬇️ 下载种子</button>
          <div style="background:#fff;border-radius:10px;padding:11px;font-size:12px;color:#667"><b>📎 简介：</b><span style="font-size:11px">{t[0]}，本站官方制作组整理，欢迎下载做种！</span></div>
''', f"E32{3+ti} · 双折叠 详情-{t[1]}"))

# ===== 14. 外屏补充 ×8 =====
def fc_shell(title, content, label, dock_icon="🔍"):
    aid = next_id()
    return f'''    <dc-artboard id="{aid}-f2c" label="{label}" width="340" height="720">
      <div class="fold2-closed">
        <div class="fc-statusbar"><span>9:41</span><span>📶 🔋</span></div>
        <div class="fc-topbar"><div class="fc-logo"><img src="{OWL}" alt=""><b>{title}</b></div><span>🔔</span></div>
        <div class="fc-scroll">{content}</div>
        <div class="fc-dock"><span>🏠</span><span style="background:#2FA8FF;color:#fff;border-radius:10px">{dock_icon}</span><span>📥</span><span>👤</span></div>
      </div>
    </dc-artboard>
'''
def fc_tor(t):
    def _tgs():
        parts = [f'<span style="background:#E8F8F2;color:#2FBF9B;border-radius:6px;padding:0 5px;font-size:8px">{x}</span>' for x in t[6].split("/")]
        return '<div style="display:flex;gap:4px;margin-top:4px">' + "".join(parts) + '</div>'
    return f'<div style="background:#fff;border-radius:10px;padding:10px;display:flex;gap:8px;align-items:center"><span style="width:28px;height:28px;border-radius:8px;background:{t[2]};color:#fff;display:flex;align-items:center;justify-content:center;font-size:11px;flex-shrink:0">{t[1]}</span><div style="flex:1;min-width:0"><b style="font-size:11px;display:block;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{t[0][:18]}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">{t[4]} · {t[5]}做种</p>{_tgs()}</div></div>'

L(fc_shell("详情", f'''
          <div style="background:#fff;border-radius:11px;padding:12px;margin-bottom:8px"><b style="font-size:12px">{TORRENTS[0][0][:24]}</b><p style="color:#99a;font-size:9px;margin:3px 0 0">gntv · 09-06 · Free</p><div style="display:flex;gap:8px;margin-top:6px"><span style="background:#E8F4FF;color:#2FA8FF;border-radius:8px;padding:2px 8px;font-size:9px">⬇️ 下载</span><span style="background:#E8F8F2;color:#2FBF9B;border-radius:8px;padding:2px 8px;font-size:9px">⭐ 收藏</span></div></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:7px;margin-bottom:8px"><div style="background:#F0F7FF;border-radius:10px;padding:9px;text-align:center"><b style="font-size:15px;color:#2FA8FF">{TORRENTS[0][5]}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">做种</p></div><div style="background:#F0F7FF;border-radius:10px;padding:9px;text-align:center"><b style="font-size:15px;color:#2FBF9B">3</b><p style="color:#99a;font-size:9px;margin:1px 0 0">下载</p></div></div>
          <div style="background:#fff;border-radius:10px;padding:10px;font-size:9px;color:#667;line-height:1.7">{TORRENTS[0][0]}，简繁双版古籍资源…</div>
''', "E329 · 双折叠 外屏-详情"))
L(fc_shell("论坛", '''
          <div style="display:grid;gap:7px"><div style="background:#fff;border-radius:10px;padding:10px"><b style="font-size:12px">💬 最新主题</b></div><div style="background:#fff;border-radius:10px;padding:10px 11px;font-size:11px"><b>IYUU辅种教程</b><p style="color:#99a;font-size:9px;margin:3px 0 0">gntv · 23回复</p></div><div style="background:#fff;border-radius:10px;padding:10px 11px;font-size:11px"><b>新人必读规则</b><p style="color:#99a;font-size:9px;margin:3px 0 0">管理组 · 45回复</p></div><div style="background:#fff;border-radius:10px;padding:10px 11px;font-size:11px"><b>求化学资料</b><p style="color:#99a;font-size:9px;margin:3px 0 0">study_mom · 8回复</p></div></div>
''', "E330 · 双折叠 外屏-论坛"))
L(fc_shell("课本", '''
          <div style="display:grid;gap:7px"><div style="display:grid;grid-template-columns:1fr 1fr;gap:7px"><div style="background:#fff;border-radius:10px;padding:11px;text-align:center"><span style="font-size:22px">📘</span><b style="display:block;font-size:10px;margin-top:2px">语文</b></div><div style="background:#fff;border-radius:10px;padding:11px;text-align:center"><span style="font-size:22px">📗</span><b style="display:block;font-size:10px;margin-top:2px">数学</b></div><div style="background:#fff;border-radius:10px;padding:11px;text-align:center"><span style="font-size:22px">📕</span><b style="display:block;font-size:10px;margin-top:2px">英语</b></div><div style="background:#fff;border-radius:10px;padding:11px;text-align:center"><span style="font-size:22px">🔬</span><b style="display:block;font-size:10px;margin-top:2px">科学</b></div><div style="background:#fff;border-radius:10px;padding:11px;text-align:center"><span style="font-size:22px">🧪</span><b style="display:block;font-size:10px;margin-top:2px">化学</b></div><div style="background:#fff;border-radius:10px;padding:11px;text-align:center"><span style="font-size:22px">🌍</span><b style="display:block;font-size:10px;margin-top:2px">地理</b></div></div></div>
''', "E331 · 双折叠 外屏-课本"))
L(fc_shell("排行", f'''
          <div style="display:grid;gap:7px">{"".join(f'<div style="display:flex;align-items:center;gap:8px;background:#fff;border-radius:10px;padding:9px 10px"><span style="width:18px;text-align:center;font-weight:700;color:{"#FFC93C" if i<3 else "#99a"}">{i+1}</span><img src="{OWL}" style="width:24px;height:24px;border-radius:50%"><b style="flex:1;font-size:11px">{u}</b><b style="font-size:10px;color:#FF7A59">{v}</b></div>' for i,(u,cl,v,_) in enumerate(RANKS[:4]))}</div>
''', "E332 · 双折叠 外屏-排行"))
L(fc_shell("商店", f'''
          <div style="display:grid;gap:7px">{"".join(f'<div style="background:#fff;border-radius:10px;padding:10px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:11px">{nm}</b><div><b style="font-size:10px;color:#FF7A59">{pr}</b><span style="background:#2FA8FF;color:#fff;border-radius:8px;padding:2px 8px;font-size:9px;margin-left:5px">买</span></div></div>' for nm,pr,_ in SHOP[:5])}</div>
''', "E333 · 双折叠 外屏-商店"))
L(fc_shell("我的", f'''
          <div style="display:grid;gap:7px"><div style="background:#fff;border-radius:11px;padding:11px;display:flex;align-items:center;gap:9px"><img src="{OWL}" style="width:32px;height:32px;border-radius:50%"><div><b style="font-size:12px">gntv</b><p style="color:#99a;font-size:9px;margin:1px 0 0">发布员 · 38.5 分享率</p></div></div>{"".join(f'<div style="background:#fff;border-radius:10px;padding:10px 11px;font-size:11px;display:flex;justify-content:space-between"><b>{nm}</b><b style="color:#2FA8FF">{v}</b></div>' for _,nm,v in MYMENU[:6])}</div>
''', "E334 · 双折叠 外屏-我的"))
L(fc_shell("公告", '''
          <div style="display:grid;gap:7px"><div style="background:#fff;border-radius:10px;padding:10px 11px"><b style="font-size:11px">📢 站免池进度通报</b><p style="color:#99a;font-size:9px;margin:3px 0 0">52.3% · 09-07</p></div><div style="background:#fff;border-radius:10px;padding:10px 11px"><b style="font-size:11px">🎉 中秋双倍免费活动</b><p style="color:#99a;font-size:9px;margin:3px 0 0">9月15-17日 · 09-15</p></div><div style="background:#fff;border-radius:10px;padding:10px 11px"><b style="font-size:11px">📚 新课本上线</b><p style="color:#99a;font-size:9px;margin:3px 0 0">人教版 2026 秋 · 09-01</p></div></div>
''', "E335 · 双折叠 外屏-公告"))
L(fc_shell("站点", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:7px;margin-bottom:7px"><div style="background:#fff;border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px;color:#2FA8FF">19,434</b><p style="color:#99a;font-size:9px;margin:1px 0 0">种子</p></div><div style="background:#fff;border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px;color:#2FA8FF">4,645</b><p style="color:#99a;font-size:9px;margin:1px 0 0">用户</p></div><div style="background:#fff;border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px;color:#2FBF9B">85.55T</b><p style="color:#99a;font-size:9px;margin:1px 0 0">总大小</p></div><div style="background:#fff;border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px;color:#2FBF9B">142,384</b><p style="color:#99a;font-size:9px;margin:1px 0 0">同伴</p></div></div>
          <div style="display:grid;gap:6px"><div style="background:#fff;border-radius:10px;padding:9px 11px;font-size:10px;display:flex;justify-content:space-between"><span>📤 总上传</span><b>4.196 PB</b></div><div style="background:#fff;border-radius:10px;padding:9px 11px;font-size:10px;display:flex;justify-content:space-between"><span>📥 总下载</span><b>467.77 TB</b></div></div>
''', "E336 · 双折叠 外屏-站点"))

# ===== 15. 半展开分屏状态 ×4 =====
def f2_split(left_title, left_items, right_title, right_items, label):
    aid = next_id()
    lc = "".join(f'<div style="background:#fff;border-radius:9px;padding:8px 10px;font-size:10px;margin-bottom:6px">{it}</div>' for it in left_items)
    rc = "".join(f'<div style="background:#fff;border-radius:9px;padding:8px 10px;font-size:10px;margin-bottom:6px">{it}</div>' for it in right_items)
    return f'''    <dc-artboard id="{aid}-f2s" label="{label}" width="720" height="900">
      <div class="fold2-screen" style="flex-direction:row">
        <div style="flex:1;background:#F5FAFF;display:flex;flex-direction:column;overflow:hidden"><div style="background:#fff;padding:9px 12px;font-size:12px;border-bottom:1px solid #EEF2F7"><b>{left_title}</b></div><div style="padding:10px;flex:1;overflow:hidden">{lc}</div></div>
        <div style="width:10px;background:linear-gradient(180deg,#2FA8FF,#5B6BF5);border-radius:5px;margin:4px 2px"></div>
        <div style="flex:1;background:#F5FAFF;display:flex;flex-direction:column;overflow:hidden"><div style="background:#fff;padding:9px 12px;font-size:12px;border-bottom:1px solid #EEF2F7"><b>{right_title}</b></div><div style="padding:10px;flex:1;overflow:hidden">{rc}</div></div>
      </div>
    </dc-artboard>
'''
split_pages = [
    ("浏览", [f"<b>{t[0][:16]}</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>{t[4]} · {t[5]}做种</p>" for t in TORRENTS[:3]], "详情", [f"<b>{TORRENTS[0][0][:18]}</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>384MB · 12做种</p>", "<b>📁 文件</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>六書正譌.txt · 128MB</p>", "<b>💬 评论</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>study_mom：扫描很清晰</p>"], "E337 · 双折叠 半开-浏览+详情"),
    ("论坛", ["<b>小学部交流</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>56帖 · 最新10:20</p>", "<b>初中部交流</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>48帖 · 最新09:45</p>"], "帖子", ["<b>IYUU辅种教程</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>356阅读 · 23回复</p>", "<b>新人必读规则</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>892阅读 · 45回复</p>"], "E338 · 双折叠 半开-论坛+帖子"),
    ("商店", [f"<b>{nm}</b><p style='color:#FF7A59;font-size:9px;margin:2px 0 0'>{pr}</p>" for nm,pr,_ in SHOP[:3]], "银行", ["<b>总资产</b><p style='font-size:12px;margin:2px 0 0'>85,814,940</p>", "<b>活期</b><p style='font-size:12px;margin:2px 0 0'>3,321.81</p>", "<b>每小时</b><p style='font-size:12px;margin:2px 0 0'>+469.144</p>"], "E339 · 双折叠 半开-商店+银行"),
    ("消息", ["<b>📥 收件箱 5</b>", "<b>📤 发件箱 3</b>", "<b>🗑️ 回收站 1</b>"], "会话", ["<b>管理组</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>您的种子已设为官种</p>", "<b>alan5914</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>求种已回复 ✓</p>"], "E340 · 双折叠 半开-消息+会话"),
]
for lt, lv, rt, rv, lb in split_pages:
    L(f2_split(lt, lv, rt, rv, lb))

html = "\n".join(out)
open('/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/gen/fold2b_boards.html','w',encoding='utf-8').write(html)
print("fold2b boards:", len(out), "last id:", n[0])

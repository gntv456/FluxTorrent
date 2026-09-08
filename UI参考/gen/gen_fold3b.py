# -*- coding: utf-8 -*-
"""三折叠第二批: 全展开细页 + 半展开 + 折叠态 + 分屏 ~95页"""
import sys
sys.path.insert(0, '/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/gen')
from data import *

n = [340]
def next_id():
    n[0] += 1
    return f"e{n[0]}"

def f3_shell(title, content, label, dark=False):
    aid = next_id()
    sb = '<div class="f3-statusbar"><span>9:41</span><span>📶 🔋 100%</span></div>'
    if dark:
        sb = '<div class="f3-statusbar" style="background:#0F172A;color:rgba(255,255,255,.6)"><span>9:41</span><span>📶 🔋 100%</span></div>'
    tb = f'<div class="f3-topbar"{" style=background:#1F2A44" if dark else ""}><span class="f3-back">‹</span><b{" style=color:#fff" if dark else ""}>{title}</b><div class="f3-search" style="visibility:hidden">🔍</div><span class="f3-filter">•••</span></div>'
    return f'''    <dc-artboard id="{aid}-f3p" label="{label}" width="1000" height="760">
      <div class="fold3-screen">
        {sb}
        {tb}
        <div class="f3-content" style="padding:16px">{content}</div>
      </div>
    </dc-artboard>
'''

def chip(t, on=False):
    return f'<span style="background:{"#2FA8FF" if on else "#F0F4F8"};color:{"#fff" if on else "#667"};border-radius:14px;padding:5px 13px;font-size:11px">{t}</span>'

def info3(title, label, items, cols=3):
    cards = "".join(f'<div style="background:#fff;border-radius:12px;padding:14px;display:flex;gap:10px;align-items:center"><span style="width:36px;height:36px;border-radius:10px;background:#F0F7FF;display:flex;align-items:center;justify-content:center;font-size:16px;flex-shrink:0">{ic}</span><div><b style="font-size:12px">{a}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">{b}</p></div></div>' for ic,a,b in items)
    L(f3_shell(title, f'<div style="display:grid;grid-template-columns:repeat({cols},1fr);gap:10px">{cards}</div>', label))

out = []
L = out.append

# ===== 1. 我的数据子菜单 ×11 =====
L(f3_shell("绩效考核界面", '''
          <div style="display:grid;grid-template-columns:1.4fr 1fr;gap:14px"><div><div style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);border-radius:16px;padding:18px;color:#fff;margin-bottom:10px"><p style="font-size:11px;opacity:.9;margin:0">保种员 5T 版 · 本月</p><b style="font-size:26px">200,000</b><p style="font-size:10px;opacity:.9;margin:3px 0 0">火花 · 考核期 09-01 ~ 09-30</p></div><div style="display:grid;grid-template-columns:1fr 1fr;gap:9px;margin-bottom:10px"><div style="background:#fff;border-radius:11px;padding:13px;text-align:center"><b style="font-size:20px;color:#2FA8FF">28/30</b><p style="color:#99a;font-size:10px;margin:2px 0 0">操作总数</p></div><div style="background:#fff;border-radius:11px;padding:13px;text-align:center"><b style="font-size:20px;color:#2FBF9B">25/30</b><p style="color:#99a;font-size:10px;margin:2px 0 0">通过审核</p></div></div></div>
          <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📜 领取历史</b><div style="display:grid;gap:6px;margin-top:9px;font-size:12px"><div style="display:flex;justify-content:space-between"><span>gntv</span><b style="color:#2FBF9B">200,000</b></div><div style="display:flex;justify-content:space-between"><span>冷冷的风</span><b style="color:#2FBF9B">200,000</b></div><div style="display:flex;justify-content:space-between"><span>alan5914</span><b style="color:#2FBF9B">200,000</b></div></div></div></div>
''', "E341 · 三折叠 绩效考核"))
L(f3_shell("装饰品管理中心", f'''
          <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:10px;margin-bottom:10px"><div style="background:linear-gradient(135deg,#FF8FC7,#FFC93C);border-radius:12px;padding:14px;color:#fff;text-align:center"><span style="font-size:26px">🖼️</span><b style="display:block;font-size:12px;margin-top:4px">星光头像框</b><p style="font-size:10px;opacity:.9;margin:2px 0 0">已装备</p></div><div style="background:#fff;border-radius:12px;padding:14px;text-align:center"><span style="font-size:26px">🌈</span><b style="display:block;font-size:12px;margin-top:4px">ID彩虹</b><p style="color:#99a;font-size:10px;margin:2px 0 0">未装备</p></div><div style="background:#fff;border-radius:12px;padding:14px;text-align:center"><span style="font-size:26px">✨</span><b style="display:block;font-size:12px;margin-top:4px">昵称发光</b><p style="color:#99a;font-size:10px;margin:2px 0 0">未购买</p></div><div style="background:#fff;border-radius:12px;padding:14px;text-align:center"><span style="font-size:26px">💎</span><b style="display:block;font-size:12px;margin-top:4px">银河边框</b><p style="color:#99a;font-size:10px;margin:2px 0 0">限定</p></div></div>
          <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:10px">{"".join(f'<div style="background:#fff;border-radius:12px;padding:13px;text-align:center"><span style="font-size:22px">{ic}</span><b style="display:block;font-size:11px;margin-top:3px">{nm}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">{ds}</p></div>' for ic,nm,ds in [("🎀","学习风背景","3,000"),("🎈","气泡装扮","2,500"),("🌟","星星挂饰","1,800"),("🪶","羽毛特效","5,000")])}</div>
''', "E342 · 三折叠 装饰品"))
info3("我的待审核", "E343 · 三折叠 待审核", [("⏳","识典古籍 六書正譌","种子 · 审核中"),("⏳","小学英语听力合集","种子 · 审核中"),("⏳","初中物理实验视频","待补充"),("✅","高中数学讲义","已通过")], cols=4)
info3("禁止中", "E344 · 三折叠 禁止中", [("🚫","暂无被禁种子","一切正常"),("✅","历史记录","无违规"),("💡","注意","遵守分享率规范")], cols=3)
info3("管理组", "E345 · 三折叠 管理组", [("🎧","一线客服","10人 · 可申请"),("💬","批评家","5人 · 可申请"),("🛡️","论坛版主","8人 · 可申请"),("⚙️","管理员","6人 · 可申请"),("👑","VIP","45人 · 可申请")], cols=5)
info3("PM 管理组", "E346 · 三折叠 PM管理组", [("📨","发给管理组","选择成员"),("📤","已发送","3条已回复"),("⏳","待回复","1条"),("🔔","通知","即时提醒")], cols=4)
info3("排行榜入口", "E347 · 三折叠 排行入口", [("📊","用户排行","上传/下载"),("🌐","社区排行","论坛活跃"),("🧲","种子排行","Torrents"),("⚡","速度排行","最快上传"),("🎖️","勋章排行","成就点")], cols=5)
info3("站点日志", "E348 · 三折叠 日志", [("📋","09-06","发布23个新种子"),("📋","09-05","更新审核规范"),("📋","09-03","修复下载计数"),("📋","09-01","站免池50%")], cols=4)
info3("守护神", "E349 · 三折叠 守护神", [("🛡️","守护神计划","长期保种10T+"),("🏆","当前排名","第12名"),("📊","我的保种","12.4TB"),("🎁","月奖励","20,000")], cols=4)
info3("发送消息", "E350 · 三折叠 发送", [("📨","收件人","gntv"),("📝","主题","谢谢资源"),("💬","内容","孩子很喜欢…"),("📤","记录","09-06")], cols=4)
info3("退出确认", "E351 · 三折叠 退出", [("👋","确定退出登录？","需重新验证"),("🔄","退出后","回到登录页"),("💡","提示","Cookie失效")], cols=3)

# ===== 2. 论坛 ×8 =====
for fi, (fn, subs, c) in enumerate(FORUMS):
    L(f3_shell(fn, f'''
          <div style="display:flex;gap:8px;margin-bottom:12px">{"".join(chip(x, on=(i==0)) for i,x in enumerate(["全部","精华","最新"]))}</div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:10px">{"".join(f'<div style="background:#fff;border-radius:11px;padding:12px 14px"><div style="display:flex;justify-content:space-between;font-size:13px"><b>{s}</b><span style="color:#99a;font-size:10px">{cnt}</span></div><p style="color:#99a;font-size:10px;margin:4px 0 0">最新：{"IYUU辅种教程" if fi==0 else "新人报道" if fi==1 else "发邀贴" if fi==2 else "脚本分享"} · 09-06</p></div>' for s,cnt in subs)}</div>
''', f"E35{2+fi} · 三折叠 论坛-{fn.replace('📢 ','').replace('📚 ','').replace('🎁 ','').replace('💻 ','')}"))
for pi, (pt, pa, pc, pl) in enumerate([
    ("IYUU辅种保姆级教程", "gntv · 发布员", "23 回复 · 356 阅读", "一步一步教你配置 IYUU，自动辅种全站，新手也能轻松上手！"),
    ("新人必读：站点规则", "管理组 · 版主", "45 回复 · 892 阅读", "欢迎来到好学PT！请先阅读站点规则，避免违规被处罚。"),
    ("求推荐高中化学资料", "study_mom · User", "8 回复 · 45 阅读", "孩子上高中了，想找一些优质的化学学习资源，求推荐！"),
]):
    L(f3_shell("帖子详情", f'''
          <div style="display:grid;grid-template-columns:1.6fr 1fr;gap:14px">
            <div><div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:14px">{pt}</b><p style="color:#99a;font-size:10px;margin:5px 0 0">{pa} · {pl}</p></div><div style="background:#fff;border-radius:12px;padding:12px;font-size:12px;color:#667;line-height:1.8;margin-bottom:10px">{pc}</div>
            <div style="display:grid;gap:8px"><div style="background:#fff;border-radius:11px;padding:11px"><div style="display:flex;gap:8px;align-items:center"><img src="{OWL}" style="width:26px;height:26px;border-radius:50%"><b style="font-size:11px">alan5914</b><span style="color:#99a;font-size:9px">09-06</span></div><p style="font-size:11px;color:#667;margin:6px 0 0">收藏了，感谢分享！</p></div><div style="background:#fff;border-radius:11px;padding:11px"><div style="display:flex;gap:8px;align-items:center"><img src="{OWL}" style="width:26px;height:26px;border-radius:50%"><b style="font-size:11px">kiririn</b><span style="color:#99a;font-size:9px">09-06</span></div><p style="font-size:11px;color:#667;margin:6px 0 0">写得很详细，赞！</p></div></div></div>
            <div style="display:grid;gap:10px;align-content:start"><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:12px;color:#8895aa">📊 帖子数据</b><div style="display:grid;gap:6px;margin-top:8px;font-size:12px"><div style="display:flex;justify-content:space-between"><span>回复</span><b>{pc.split(" · ")[0]}</b></div><div style="display:flex;justify-content:space-between"><span>阅读</span><b>{pc.split(" · ")[1]}</b></div></div></div><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:12px;color:#8895aa">👤 楼主</b><div style="display:flex;gap:8px;align-items:center;margin-top:8px"><img src="{OWL}" style="width:32px;height:32px;border-radius:50%"><div><b style="font-size:12px">{pa.split(" · ")[0]}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">{pa.split(" · ")[1]}</p></div></div></div><div style="background:#E8F4FF;border-radius:12px;padding:14px;font-size:11px;color:#2FA8FF">💡 文明发言，友善交流，共建美好学习社区！</div></div>
          </div>
''', f"E35{6+pi} · 三折叠 帖子详情-{pt[:6]}"))
L(f3_shell("发帖", '''
          <div style="display:grid;grid-template-columns:1.4fr 1fr;gap:14px;max-width:880px;margin:0 auto">
            <div style="background:#fff;border-radius:14px;padding:18px;display:grid;gap:12px"><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">版块 *</label><select class="gi" style="width:100%;height:38px;font-size:12px"><option>学习交流俱乐部 · 小学部交流</option><option>本站事务区</option><option>发邀专区</option><option>技术交流</option></select></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">标题 *</label><input class="gi" placeholder="请输入帖子标题" style="width:100%;height:38px;font-size:13px"></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">内容 *</label><textarea class="gi" placeholder="请输入帖子内容…" style="width:100%;height:110px;font-size:12px;resize:none"></textarea></div><div style="display:flex;gap:6px;flex-wrap:wrap"><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 11px;font-size:10px">#求助</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 11px;font-size:10px">#分享</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 11px;font-size:10px">#讨论</span></div></div>
            <div style="display:grid;gap:10px;align-content:start"><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:12px;color:#8895aa">📋 发帖规范</b><div style="display:grid;gap:7px;margin-top:9px;font-size:11px;color:#667"><div>✅ 标题清晰明确</div><div>✅ 内容充实有用</div><div>✅ 禁止广告与违规</div></div></div><button class="f3-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;width:100%;padding:12px;border-radius:11px;font-size:14px">📤 发布帖子</button></div>
          </div>
''', "E359 · 三折叠 发帖"))

# ===== 3. 课本管理 ×6 =====
info3("我的课本提交", "E360 · 三折叠 我的提交", [("📘","人教版语文四年级上","已通过 09-06"),("📗","人教版数学五年级","审核中 09-05"),("📙","部编版语文三年级","已通过 08-30"),("➕","提交新课本","选择科目版本")], cols=4)
info3("课本管理后台", "E361 · 三折叠 管理课本", [("⚙️","科目管理","29 个科目"),("📚","课本管理","1,234 本"),("⏳","待审核","5 本"),("📊","下载统计","45,892次")], cols=4)
L(f3_shell("创建课本", f'''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:14px;max-width:840px;margin:0 auto">
            <div style="background:#fff;border-radius:14px;padding:18px;display:grid;gap:12px"><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">科目 *</label><select class="gi" style="width:100%;height:38px;font-size:12px">{"".join(f"<option>{s}</option>" for s in SUBJECTS[:8])}</select></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">版本 *</label><select class="gi" style="width:100%;height:38px;font-size:12px"><option>人教版</option><option>部编版</option><option>北师大版</option><option>苏教版</option></select></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">简介</label><textarea class="gi" placeholder="课本说明…" style="width:100%;height:70px;font-size:12px;resize:none"></textarea></div></div>
            <div style="background:#fff;border-radius:14px;padding:18px"><b style="font-size:13px">📚 年级选择</b><div style="display:flex;gap:6px;flex-wrap:wrap;margin-top:10px">{"".join(chip(x, on=(i==0)) for i,x in enumerate(["一年级","二年级","三年级","四年级","五年级","六年级","初一","初二","初三","高一","高二","高三"]))}</div><button class="f3-btn" style="background:#2FBF9B;color:#fff;border:none;width:100%;margin-top:16px;padding:12px;border-radius:11px;font-size:14px">➕ 创建课本</button></div>
          </div>
''', "E362 · 三折叠 创建课本"))
for gi, (gn, subs) in enumerate([("小学", SUBJECTS[:6]), ("初中", SUBJECTS[4:10]), ("高中", SUBJECTS[8:14])]):
    L(f3_shell(f"课本-{gn}", f'''
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:10px">{"".join(f'<div style="background:#fff;border-radius:12px;padding:14px;text-align:center"><span style="font-size:26px">📘</span><b style="display:block;font-size:13px;margin-top:4px">{s}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">人教版 · 12本 · 356次下载</p><span style="color:#2FA8FF;font-size:11px;display:inline-block;margin-top:5px">查看 ›</span></div>' for s in subs)}</div>
''', f"E36{3+gi} · 三折叠 课本-{gn}"))

# ===== 4. 勋章分类 ×6 =====
medal_cats = [
    ("二十四节气", [("🌸","立春"),("🌧️","雨水"),("⚡","惊蛰"),("🌿","春分"),("🔥","立夏"),("🌾","芒种")]),
    ("学习课堂", [("📚","初出茅庐"),("✏️","勤学苦练"),("🏆","学霸之路"),("🎓","金榜题名"),("📖","书香门第"),("🧠","最强大脑")]),
    ("节日系列", [("🎆","开站纪念"),("🏮","春节限定"),("🥮","中秋限定"),("🎃","万圣节"),("🎄","圣诞限定"),("🎊","新年限定")]),
    ("开站系列", [("🌟","首批用户"),("🚀","开站元勋"),("💎","创始会员"),("🎖️","一周年"),("⏳","两周年"),("🔥","三周年")]),
    ("工作组", [("🛡️","维护先锋"),("🎧","客服之星"),("🖥️","技术大牛"),("📤","发布达人"),("🎨","设计高手"),("⚙️","运维专家")]),
    ("未分类", [("🎲","幸运儿"),("🎯","签到狂魔"),("🦉","猫头鹰之友"),("🌟","学习之星"),("🌈","彩虹收藏家"),("🎁","锦鲤体质")]),
]
for mi, (cn, items) in enumerate(medal_cats):
    L(f3_shell(f"勋章-{cn}", f'''
          <div style="background:#fff;border-radius:12px;padding:13px;margin-bottom:12px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:14px">{cn}</b><span style="color:#99a;font-size:11px">已收集 {len(items)//2} / {len(items)} 枚</span></div>
          <div style="display:grid;grid-template-columns:repeat(6,1fr);gap:10px">{"".join(f'<div style="background:#fff;border-radius:12px;padding:13px;text-align:center;{"border:2px solid #E8F4FF" if i%2==0 else "opacity:.45"}"><span style="font-size:26px">{m}</span><b style="display:block;font-size:11px;margin-top:3px">{nm}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">{cn}</p></div>' for i,(m,nm) in enumerate(items))}</div>
''', f"E36{6+mi} · 三折叠 勋章-{cn}"))

# ===== 5. 排行子类 ×8 =====
rank_subs = [
    ("上传者排行", "上传量", [("dgvge","109.697 TB"),("Rick","101.492 TB"),("kiririn","81.859 TB")]),
    ("下载者排行", "下载量", [("shmt86","56.2 TB"),("JaCksonPp","42.8 TB"),("gntv","2.83 TB")]),
    ("做种数排行", "做种数", [("gntv","16,348"),("alan5914","1,095"),("ranfish","365")]),
    ("保种大小排行", "保种大小", [("dgvge","85.4 TB"),("Rick","76.2 TB"),("kiririn","61.9 TB")]),
    ("最快上传者", "速度", [("dgvge","38.2 MB/s"),("Rick","29.6 MB/s"),("shmt86","24.1 MB/s")]),
    ("最快下载者", "速度", [("JaCksonPp","42.5 MB/s"),("shmt86","35.8 MB/s"),("kiririn","22.9 MB/s")]),
    ("最佳分享者", "分享率", [("dgvge","99.9"),("Rick","88.4"),("gntv","38.5")]),
    ("最差分享者", "分享率", [("user_a","0.12"),("user_b","0.21"),("user_c","0.28")]),
]
for ri, (rn, col, rows_data) in enumerate(rank_subs):
    rows = "".join(f'<div style="display:flex;align-items:center;gap:10px;padding:10px 12px;background:{"linear-gradient(135deg,#FFF8E8,#FFF4D6)" if i==0 else "#fff"};border-radius:11px"><span style="width:22px;text-align:center;font-weight:700;color:{"#FFC93C" if i<3 else "#99a"}">{i+1}</span><img src="{OWL}" style="width:30px;height:30px;border-radius:50%"><div style="flex:1"><b style="font-size:12px">{u}</b><p style="color:#99a;font-size:9px;margin:0">{rn}</p></div><b style="font-size:12px;color:{"#FF7A59" if i==0 else "#1F2A44"}">{v}</b></div>' for i,(u,v) in enumerate(rows_data))
    L(f3_shell("排行榜", f'''
          <div style="display:flex;gap:8px;margin-bottom:12px;flex-wrap:wrap">{"".join(chip(x, on=(x==rn)) for x in ["上传者","下载者","做种","保种大小","最快上传","最快下载","最佳分享","最差分享"])}</div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px"><div style="display:grid;gap:8px">{rows}</div><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📊 {col}统计</b><div style="display:grid;gap:8px;margin-top:10px;font-size:12px"><div style="display:flex;justify-content:space-between;padding:8px 0;border-bottom:1px solid #F0F4F8"><span style="color:#99a">TOP1 {col}</span><b>{rows_data[0][1]}</b></div><div style="display:flex;justify-content:space-between;padding:8px 0;border-bottom:1px solid #F0F4F8"><span style="color:#99a">TOP3 均值</span><b>{(rows_data[0][1]+rows_data[1][1]+rows_data[2][1]).replace(" ","")}</b></div><div style="display:flex;justify-content:space-between;padding:8px 0"><span style="color:#99a">上榜门槛</span><b>62.1 TB</b></div></div></div></div>
''', f"E37{2+ri} · 三折叠 排行-{rn}"))

# ===== 6. 游戏 ×5 =====
L(f3_shell("五子棋", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:14px;max-width:760px;margin:0 auto"><div style="background:#fff;border-radius:12px;padding:14px;text-align:center"><b style="font-size:14px">⚫ 五子棋 · 你执黑</b><p style="color:#99a;font-size:10px;margin:3px 0 0">18 胜 12 负 · 回合 12</p><div style="background:#F5E8C8;border-radius:8px;padding:10px;margin-top:10px;display:grid;grid-template-columns:repeat(5,1fr);gap:0;max-width:320px;margin:10px auto 0"><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A;border-radius:50%;box-shadow:inset 0 0 0 3px #2FA8FF"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A;border-radius:50%"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A;border-radius:50%;box-shadow:inset 0 0 0 3px #1F2A44"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div></div></div>
          <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📊 战绩</b><div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:8px;margin-top:10px;text-align:center"><div><b style="font-size:16px;color:#2FBF9B">18</b><p style="color:#99a;font-size:9px;margin:0">胜</p></div><div><b style="font-size:16px;color:#FF7A59">12</b><p style="color:#99a;font-size:9px;margin:0">负</p></div><div><b style="font-size:16px">1,260</b><p style="color:#99a;font-size:9px;margin:0">积分</p></div></div><button class="f3-btn" style="background:#2FA8FF;color:#fff;border:none;width:100%;margin-top:12px;padding:11px;border-radius:10px;font-size:13px">匹配对手</button></div></div>
''', "E380 · 三折叠 五子棋"))
L(f3_shell("刮刮乐", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:14px;max-width:760px;margin:0 auto"><div style="background:linear-gradient(135deg,#FF7A59,#FFC93C);border-radius:14px;padding:16px;color:#fff;text-align:center"><b style="font-size:15px">🎫 刮刮乐</b><p style="font-size:11px;opacity:.9;margin:4px 0 0">2,000 火花 / 次 · 最高赢 50,000</p></div>
          <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">🎁 刮奖区</b><div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin-top:10px"><div style="background:linear-gradient(135deg,#B8C4D9,#8B9BB8);border-radius:10px;height:64px;display:flex;align-items:center;justify-content:center;font-size:20px;color:#fff">🎁</div><div style="background:linear-gradient(135deg,#FFD54F,#FFB300);border-radius:10px;height:64px;display:flex;align-items:center;justify-content:center;font-size:20px;color:#fff">🎉</div><div style="background:linear-gradient(135deg,#B8C4D9,#8B9BB8);border-radius:10px;height:64px;display:flex;align-items:center;justify-content:center;font-size:20px;color:#fff">🎁</div><div style="background:linear-gradient(135deg,#B8C4D9,#8B9BB8);border-radius:10px;height:64px;display:flex;align-items:center;justify-content:center;font-size:20px;color:#fff">🎁</div><div style="background:linear-gradient(135deg,#B8C4D9,#8B9BB8);border-radius:10px;height:64px;display:flex;align-items:center;justify-content:center;font-size:20px;color:#fff">🎁</div><div style="background:linear-gradient(135deg,#B8C4D9,#8B9BB8);border-radius:10px;height:64px;display:flex;align-items:center;justify-content:center;font-size:20px;color:#fff">🎁</div></div><div style="background:#FFF8E8;border-radius:9px;padding:9px;margin-top:10px;font-size:11px;color:#667">🎉 刮出「好运」！+2,000 火花</div><button class="f3-btn" style="background:#FF7A59;color:#fff;border:none;width:100%;margin-top:10px;padding:11px;border-radius:10px;font-size:13px">🎫 再刮一次</button></div></div>
''', "E381 · 三折叠 刮刮乐"))
L(f3_shell("九宫格抽奖", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:14px;max-width:760px;margin:0 auto"><div style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);border-radius:14px;padding:16px;color:#fff;text-align:center"><b style="font-size:15px">🎯 九宫格抽奖</b><p style="font-size:11px;opacity:.9;margin:4px 0 0">1,000 火花 / 次 · 大奖 100,000</p></div>
          <div style="background:#fff;border-radius:12px;padding:14px"><div style="display:grid;grid-template-columns:repeat(3,1fr);gap:6px"><div style="background:#fff;border:1px solid #EEF2F7;border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px">📚</div><div style="background:#fff;border:1px solid #EEF2F7;border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px">💰</div><div style="background:#fff;border:1px solid #EEF2F7;border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px">🎖️</div><div style="background:#fff;border:1px solid #EEF2F7;border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px">🎁</div><div style="background:#2FA8FF;border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px;color:#fff;box-shadow:0 0 16px rgba(47,168,255,.6)">🎯</div><div style="background:#fff;border:1px solid #EEF2F7;border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px">⭐</div><div style="background:#fff;border:1px solid #EEF2F7;border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px">💎</div><div style="background:#fff;border:1px solid #EEF2F7;border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px">🔥</div><div style="background:#fff;border:1px solid #EEF2F7;border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px">🌈</div></div><button class="f3-btn" style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);color:#fff;border:none;width:100%;margin-top:12px;padding:11px;border-radius:10px;font-size:13px">🎯 开始抽奖</button></div></div>
''', "E382 · 三折叠 九宫格"))
L(f3_shell("猜大小", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:14px;max-width:760px;margin:0 auto"><div style="background:linear-gradient(135deg,#2FBF9B,#2FA8FF);border-radius:14px;padding:16px;color:#fff;text-align:center"><b style="font-size:15px">🎲 猜大小</b><p style="font-size:11px;opacity:.9;margin:4px 0 0">押注 500~50,000 · 1.95 倍</p></div>
          <div style="background:#fff;border-radius:12px;padding:14px"><div style="text-align:center"><p style="color:#99a;font-size:10px;margin:0">本期点数</p><b style="font-size:34px">12</b><div style="display:flex;gap:8px;justify-content:center;margin-top:6px"><span style="font-size:26px">⚀</span><span style="font-size:26px">⚂</span><span style="font-size:26px">⚄</span></div><p style="font-size:12px;margin:6px 0 0;color:#FF7A59">大！</p></div><div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-top:12px"><div style="background:#E8F4FF;border-radius:11px;padding:11px;text-align:center"><b style="font-size:14px;color:#2FA8FF">押 大</b></div><div style="background:#FFE8F5;border-radius:11px;padding:11px;text-align:center"><b style="font-size:14px;color:#FF8FC7">押 小</b></div></div><div style="background:#fff;border-radius:9px;padding:9px;margin-top:10px;font-size:11px;color:#99a">🎉 上局：押大 +1,950</div></div></div>
''', "E383 · 三折叠 猜大小"))
L(f3_shell("卡牌合成", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:14px;max-width:760px;margin:0 auto"><div style="background:linear-gradient(135deg,#8B5CF6,#5B6BF5);border-radius:14px;padding:16px;color:#fff;text-align:center"><b style="font-size:15px">🃏 卡牌合成</b><p style="font-size:11px;opacity:.9;margin:4px 0 0">3 张同阶合成 1 张高阶</p></div>
          <div style="background:#fff;border-radius:12px;padding:14px"><div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px"><div style="background:linear-gradient(135deg,#E8F4FF,#D6E9FF);border-radius:12px;padding:12px;text-align:center"><span style="font-size:28px">🦉</span><b style="display:block;font-size:10px;margin-top:2px">猫头鹰·N</b></div><div style="background:linear-gradient(135deg,#E8F4FF,#D6E9FF);border-radius:12px;padding:12px;text-align:center"><span style="font-size:28px">🦉</span><b style="display:block;font-size:10px;margin-top:2px">猫头鹰·N</b></div><div style="background:linear-gradient(135deg,#E8F4FF,#D6E9FF);border-radius:12px;padding:12px;text-align:center"><span style="font-size:28px">🦉</span><b style="display:block;font-size:10px;margin-top:2px">猫头鹰·N</b></div></div><div style="background:#fff;border-radius:11px;padding:12px;margin-top:10px;text-align:center;border:2px dashed #8B5CF6"><span style="font-size:14px;color:#8B5CF6">＋ 合成 →</span><div style="font-size:22px;margin-top:2px">📘</div><b style="font-size:11px">古籍·R</b></div><button class="f3-btn" style="background:#8B5CF6;color:#fff;border:none;width:100%;margin-top:10px;padding:11px;border-radius:10px;font-size:13px">✨ 合成</button></div></div>
''', "E384 · 三折叠 卡牌合成"))

# ===== 7. 工具箱 ×6 =====
tool_pages = [
    ("获取图床Token", [("🔑","Token 已获取","有效期 30 天"),("📋","Token","hxpt_img_8f3a9c…"),("💡","用途","官方图床 API 上传"),("🔄","刷新","点击重新生成")]),
    ("官方图床", [("🖼️","上传图片","jpg/png/webp ≤10MB"),("📤","最近上传","6 张 · 今天"),("🔗","外链格式","https://img.hxpt.org/…"),("💡","建议","发布资源用图床外链")]),
    ("图床批量上传", [("📁","批量上传","一次最多20张"),("⏳","进度","8/20"),("✅","成功","8 张"),("❌","失败","0 张")]),
    ("AutoFeed 转种", [("⚙️","AutoFeed","自动抓取转种"),("🔄","状态","已停止"),("📡","最近抓取","09-06 · 12个"),("⚙️","配置","RSS源与规则")]),
    ("IYUU 辅种", [("🔁","IYUU","跨站自动辅种"),("🔄","状态","已开启"),("🎯","辅种数量","1,234 个"),("📊","今日收益","+8.2GB")]),
    ("照片压缩", [("🖼️","压缩图片","jpg/png ≤30MB"),("📉","压缩率","最大80%"),("✅","最近处理","12 张"),("💡","提示","发布前先压缩")]),
]
for ti, (tn, items) in enumerate(tool_pages):
    info3(tn, f"E38{5+ti} · 三折叠 {tn}", items, cols=4)

# ===== 8. 浏览状态 ×4 =====
L(f3_shell("浏览-排序", f'''
          <div style="display:flex;gap:8px;margin-bottom:12px;flex-wrap:wrap">{"".join(chip(x, on=(i==0)) for i,x in enumerate(["最新","热门","大小","做种数","发布时间"]))}</div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:10px">{"".join(f'<div style="background:#fff;border-radius:11px;padding:12px;display:flex;gap:10px;align-items:center"><span style="width:34px;height:34px;border-radius:9px;background:{t[2]};color:#fff;display:flex;align-items:center;justify-content:center;font-size:13px;flex-shrink:0">{t[1]}</span><div style="flex:1;min-width:0"><b style="font-size:12px;display:block;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{t[0][:22]}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">{t[4]} · {t[5]}做种</p></div></div>' for t in TORRENTS[:6])}</div>
''', "E391 · 三折叠 浏览-排序"))
L(f3_shell("浏览-筛选", '''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:12px;display:grid;gap:10px"><div><label style="font-size:10px;color:#99a">分类</label><div style="display:flex;gap:6px;flex-wrap:wrap;margin-top:4px"><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px;font-size:10px">全部</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px;font-size:10px">学前教育</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px;font-size:10px">小学</span><span style="background:#2FA8FF;color:#fff;border-radius:14px;padding:5px 12px;font-size:10px">初中</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px;font-size:10px">职高</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px;font-size:10px">高中</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px;font-size:10px">教育影音</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px;font-size:10px">纪录片</span></div></div><div><label style="font-size:10px;color:#99a">标签</label><div style="display:flex;gap:6px;flex-wrap:wrap;margin-top:4px"><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px;font-size:10px">免费</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px;font-size:10px">保种</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px;font-size:10px">官方</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px;font-size:10px">2x</span></div></div></div>
          <div style="display:flex;gap:10px"><button class="f3-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;flex:1;padding:12px;border-radius:11px;font-size:14px">🔍 应用筛选</button><button class="f3-btn" style="background:#F0F4F8;color:#667;border:none;flex:1;padding:12px;border-radius:11px;font-size:14px">清空</button></div>
          <p style="text-align:center;color:#99a;font-size:11px;margin-top:10px">共 3,421 个种子符合条件</p>
''', "E392 · 三折叠 浏览-筛选"))
L(f3_shell("浏览-分页", f'''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:10px;margin-bottom:12px">{"".join(f'<div style="background:#fff;border-radius:11px;padding:11px;display:flex;gap:9px;align-items:center"><span style="width:32px;height:32px;border-radius:9px;background:{t[2]};color:#fff;display:flex;align-items:center;justify-content:center;font-size:12px">{t[1]}</span><div style="flex:1;min-width:0"><b style="font-size:11px;display:block;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{t[0][:20]}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">{t[5]}做种</p></div></div>' for t in TORRENTS[:6])}</div>
          <div style="display:flex;justify-content:center;gap:7px;align-items:center;font-size:12px"><span style="background:#F0F4F8;color:#99a;border-radius:9px;padding:6px 11px">‹</span><span style="background:#2FA8FF;color:#fff;border-radius:9px;padding:6px 12px;font-weight:700">1</span><span style="background:#F0F4F8;color:#667;border-radius:9px;padding:6px 12px">2</span><span style="background:#F0F4F8;color:#667;border-radius:9px;padding:6px 12px">3</span><span style="background:#F0F4F8;color:#99a;border-radius:9px;padding:6px 12px">…</span><span style="background:#F0F4F8;color:#667;border-radius:9px;padding:6px 12px">486</span><span style="background:#F0F4F8;color:#99a;border-radius:9px;padding:6px 11px">›</span></div>
''', "E393 · 三折叠 浏览-分页"))
L(f3_shell("浏览-网格", f'''
          <div style="display:flex;gap:8px;margin-bottom:12px"><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 13px;font-size:11px">☰ 列表</span><span style="background:#E8F4FF;color:#2FA8FF;border-radius:14px;padding:5px 13px;font-size:11px">▦ 网格</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 13px;font-size:11px">🖼️ 大图</span></div>
          <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:10px">{"".join(f'<div style="background:#fff;border-radius:12px;padding:13px;text-align:center"><span style="font-size:26px">{t[1]}</span><b style="display:block;font-size:11px;margin-top:4px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{t[0][:14]}</b><p style="color:#99a;font-size:9px;margin:2px 0 0">{t[4]} · {t[5]}做种</p></div>' for t in TORRENTS[:8])}</div>
''', "E394 · 三折叠 浏览-网格"))

# ===== 9. 详情页签 ×5 =====
L(f3_shell("详情-简介", f'''
          <div style="display:grid;grid-template-columns:1.6fr 1fr;gap:14px"><div><div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><div style="display:flex;gap:8px;align-items:flex-start"><b style="font-size:15px;flex:1">{TORRENTS[0][0]}</b><span class="tag tag-free">Free</span></div><p style="color:#99a;font-size:10px;margin:5px 0 0">发布者 gntv · 2026-09-06</p></div>
          <div style="display:flex;gap:5px;margin-bottom:10px;font-size:10px"><span style="background:#E8F4FF;color:#2FA8FF;border-radius:12px;padding:5px 13px;font-weight:600">📝 简介</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 13px">📁 文件</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 13px">🔄 做种</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 13px">💬 评论</span></div>
          <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:12px;color:#8895aa">资源简介</b><p style="font-size:12px;color:#667;line-height:2;margin:8px 0 0">识典古籍系列收录中华经典古籍的简繁双版扫描资源，含 TXT、PDF、EPUB 格式及 WEBP 扫描页，适合古文学习与研究使用。本站官方制作组 HX 出品。</p><div style="background:#F8FAFC;border-radius:9px;padding:10px;margin-top:10px;font-size:11px;color:#667"><b>📋 制作说明</b><br>扫描页 44 张 · OCR 校对 · 简繁对照</div></div></div>
          <div style="display:grid;gap:10px;align-content:start"><div style="background:#F0F7FF;border-radius:12px;padding:14px;text-align:center"><b style="font-size:22px;color:#2FA8FF">{TORRENTS[0][5]}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">做种</p></div><button class="f3-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;width:100%;padding:13px;border-radius:11px;font-size:14px">⬇️ 下载种子</button><div style="background:#fff;border-radius:12px;padding:14px;font-size:11px;color:#667"><b>📎 种子链接</b><p style="color:#2FA8FF;margin:4px 0 0;font-size:10px">https://www.hxpt.org/download.php?id=19605</p></div></div></div>
''', "E395 · 三折叠 详情-简介"))
L(f3_shell("详情-文件", f'''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:14px"><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📁 文件列表（48）</b><div style="display:grid;gap:5px;margin-top:9px;font-size:12px"><div style="display:flex;justify-content:space-between;padding:8px 0;border-bottom:1px solid #F0F4F8"><span>📄 六書正譌.txt</span><span style="color:#99a">128MB</span></div><div style="display:flex;justify-content:space-between;padding:8px 0;border-bottom:1px solid #F0F4F8"><span>📄 六書正譌.pdf</span><span style="color:#99a">96MB</span></div><div style="display:flex;justify-content:space-between;padding:8px 0;border-bottom:1px solid #F0F4F8"><span>📄 六書正譌.epub</span><span style="color:#99a">12MB</span></div><div style="display:flex;justify-content:space-between;padding:8px 0;border-bottom:1px solid #F0F4F8"><span>📄 六書正譌.txt</span><span style="color:#99a">128MB</span></div><div style="display:flex;justify-content:space-between;padding:8px 0;border-bottom:1px solid #F0F4F8"><span>📁 扫描页/</span><span style="color:#99a">44文件</span></div><div style="display:flex;justify-content:space-between;padding:8px 0"><span>📁 简繁对照/</span><span style="color:#99a">2文件</span></div></div></div>
          <div style="display:grid;gap:10px;align-content:start"><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📊 统计</b><div style="display:grid;gap:6px;margin-top:8px;font-size:12px"><div style="display:flex;justify-content:space-between"><span style="color:#99a">文件总数</span><b>48</b></div><div style="display:flex;justify-content:space-between"><span style="color:#99a">总大小</span><b>384 MB</b></div><div style="display:flex;justify-content:space-between"><span style="color:#99a">格式</span><b>TXT/PDF/EPUB/WEBP</b></div></div></div><button class="f3-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;width:100%;padding:13px;border-radius:11px;font-size:14px">⬇️ 全部下载</button></div></div>
''', "E396 · 三折叠 详情-文件"))
L(f3_shell("详情-做种", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:12px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:14px">🔄 做种列表</b><p style="color:#99a;font-size:10px;margin:3px 0 0">12 位同学正在做种 · 合计 2.1 MB/s</p></div><span style="background:#E8F4FF;color:#2FA8FF;border-radius:10px;padding:6px 13px;font-size:11px">我也来做种</span></div>
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:10px">{"".join(f'<div style="background:#fff;border-radius:12px;padding:13px"><div style="display:flex;gap:9px;align-items:center"><img src="{OWL}" style="width:30px;height:30px;border-radius:50%"><div><b style="font-size:12px">{u}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">{cl}</p></div></div><div style="display:flex;justify-content:space-between;margin-top:9px;font-size:11px"><span style="color:#99a">上传</span><b style="color:#2FBF9B">{v}</b></div></div>' for u,cl,v in [("gntv","发布员","1.2 MB/s"),("alan5914","上传者","0.8 MB/s"),("study_mom","User","0.1 MB/s"),("kiririn","Elite","0.6 MB/s"),("shmt86","Power","0.4 MB/s"),("JaCksonPp","Crazy","0.3 MB/s")])}</div>
''', "E397 · 三折叠 详情-做种"))
L(f3_shell("详情-下载", '''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:12px"><b style="font-size:14px">📥 下载列表</b><p style="color:#99a;font-size:10px;margin:3px 0 0">3 位同学正在下载</p></div>
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:10px"><div style="background:#fff;border-radius:12px;padding:13px"><div style="display:flex;gap:9px;align-items:center"><img src="''' + OWL + '''" style="width:30px;height:30px;border-radius:50%"><b style="font-size:12px">kiririn</b></div><div style="display:flex;justify-content:space-between;margin-top:9px;font-size:11px"><span style="color:#99a">速度</span><b style="color:#2FA8FF">1.8 MB/s</b></div><div style="height:6px;background:#F0F4F8;border-radius:4px;margin-top:6px;overflow:hidden"><div style="width:65%;height:100%;background:#2FA8FF"></div></div><p style="color:#99a;font-size:9px;margin:3px 0 0">65%</p></div><div style="background:#fff;border-radius:12px;padding:13px"><div style="display:flex;gap:9px;align-items:center"><img src="''' + OWL + '''" style="width:30px;height:30px;border-radius:50%"><b style="font-size:12px">shmt86</b></div><div style="display:flex;justify-content:space-between;margin-top:9px;font-size:11px"><span style="color:#99a">速度</span><b style="color:#2FA8FF">0.9 MB/s</b></div><div style="height:6px;background:#F0F4F8;border-radius:4px;margin-top:6px;overflow:hidden"><div style="width:32%;height:100%;background:#2FA8FF"></div></div><p style="color:#99a;font-size:9px;margin:3px 0 0">32%</p></div><div style="background:#fff;border-radius:12px;padding:13px"><div style="display:flex;gap:9px;align-items:center"><img src="''' + OWL + '''" style="width:30px;height:30px;border-radius:50%"><b style="font-size:12px">JaCksonPp</b></div><div style="display:flex;justify-content:space-between;margin-top:9px;font-size:11px"><span style="color:#99a">状态</span><b style="color:#99a">排队中</b></div><div style="height:6px;background:#F0F4F8;border-radius:4px;margin-top:6px;overflow:hidden"><div style="width:0%;height:100%"></div></div><p style="color:#99a;font-size:9px;margin:3px 0 0">等待</p></div></div>
''', "E398 · 三折叠 详情-下载"))
L(f3_shell("详情-评论", '''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:12px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:14px">💬 评论（12）</b><p style="color:#99a;font-size:10px;margin:3px 0 0">4.9 分 · 多数好评</p></div><span style="color:#FFC93C;font-size:16px">★★★★★</span></div>
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:10px;margin-bottom:12px"><div style="background:#fff;border-radius:12px;padding:12px"><div style="display:flex;gap:8px;align-items:center"><img src="''' + OWL + '''" style="width:26px;height:26px;border-radius:50%"><b style="font-size:11px">study_mom</b><span style="color:#99a;font-size:9px;margin-left:auto">09-06</span></div><p style="font-size:11px;color:#667;margin:6px 0 0;line-height:1.6">扫描页很清晰，孩子学习古文太方便了！</p></div><div style="background:#fff;border-radius:12px;padding:12px"><div style="display:flex;gap:8px;align-items:center"><img src="''' + OWL + '''" style="width:26px;height:26px;border-radius:50%"><b style="font-size:11px">kiririn</b><span style="color:#99a;font-size:9px;margin-left:auto">09-06</span></div><p style="font-size:11px;color:#667;margin:6px 0 0;line-height:1.6">EPUB 版本体验极佳，感谢制作组！</p></div><div style="background:#fff;border-radius:12px;padding:12px"><div style="display:flex;gap:8px;align-items:center"><img src="''' + OWL + '''" style="width:26px;height:26px;border-radius:50%"><b style="font-size:11px">alan5914</b><span style="color:#99a;font-size:9px;margin-left:auto">09-05</span></div><p style="font-size:11px;color:#667;margin:6px 0 0;line-height:1.6">简繁对照功能太贴心啦！</p></div></div>
          <div style="background:#fff;border-radius:12px;padding:12px;display:flex;gap:9px"><input class="gi" placeholder="写下你的评价…" style="flex:1;height:38px;font-size:12px"><span style="background:#2FA8FF;color:#fff;border-radius:10px;padding:0 18px;display:flex;align-items:center;font-size:12px">发送</span></div>
''', "E399 · 三折叠 详情-评论"))

# ===== 10. 请求/字幕/保种/官种详情 ×4 =====
L(f3_shell("求种详情", '''
          <div style="display:grid;grid-template-columns:1.6fr 1fr;gap:14px"><div><div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:14px">🙋 小学数学思维训练 4年级</b><p style="color:#99a;font-size:10px;margin:5px 0 0">study_mom 发起 · 09-05 · 悬赏 5,000 火花</p></div><div style="background:#fff;border-radius:12px;padding:12px;font-size:12px;color:#667;line-height:1.9;margin-bottom:10px">想要一套 4 年级的数学思维训练题集，最好是 PDF 扫描版，孩子暑假在家练习用。</div><div style="display:grid;grid-template-columns:1fr 1fr;gap:9px"><div style="background:#F0F7FF;border-radius:10px;padding:11px;text-align:center"><b style="font-size:18px;color:#2FA8FF">3</b><p style="color:#99a;font-size:10px;margin:2px 0 0">人已求</p></div><div style="background:#FFF8E8;border-radius:10px;padding:11px;text-align:center"><b style="font-size:18px;color:#FF7A59">5,000</b><p style="color:#99a;font-size:10px;margin:2px 0 0">悬赏火花</p></div></div></div>
          <div style="display:grid;gap:10px;align-content:start"><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:12px;color:#8895aa">🙏 求种者</b><div style="display:flex;gap:9px;align-items:center;margin-top:8px"><img src="''' + OWL + '''" style="width:30px;height:30px;border-radius:50%"><div><b style="font-size:12px">study_mom</b><p style="color:#99a;font-size:9px;margin:1px 0 0">User · 上传892</p></div></div></div><button class="f3-btn" style="background:#FF7A59;color:#fff;border:none;width:100%;padding:13px;border-radius:11px;font-size:14px">💛 我也要求</button></div></div>
''', "E400 · 三折叠 求种详情"))
L(f3_shell("字幕详情", '''
          <div style="display:grid;grid-template-columns:1.6fr 1fr;gap:14px"><div><div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:14px">🎬 蓝色星球 E01 中英双语</b><p style="color:#99a;font-size:10px;margin:5px 0 0">kiririn 上传 · 09-06 · ASS</p></div><div style="background:#fff;border-radius:12px;padding:12px;font-size:12px;color:#667;line-height:1.9;margin-bottom:10px">对应「蓝色星球 第三季」第 1 集，中英双语对照字幕，含特效样式，适配 1080p/4K 版本。</div><button class="f3-btn" style="background:linear-gradient(135deg,#2FBF9B,#2FA8FF);color:#fff;border:none;width:100%;padding:13px;border-radius:11px;font-size:14px">⬇️ 下载字幕</button></div>
          <div style="display:grid;gap:10px;align-content:start"><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:12px;color:#8895aa">📊 字幕信息</b><div style="display:grid;gap:6px;margin-top:8px;font-size:12px"><div style="display:flex;justify-content:space-between"><span style="color:#99a">下载</span><b>2,304</b></div><div style="display:flex;justify-content:space-between"><span style="color:#99a">语言</span><b>双语</b></div><div style="display:flex;justify-content:space-between"><span style="color:#99a">格式</span><b>ASS</b></div></div></div><div style="background:#E8F8F2;border-radius:12px;padding:14px;font-size:11px;color:#2FBF9B">💡 关联种子：蓝色星球 第三季 · 豆瓣 9.8</div></div></div>
''', "E401 · 三折叠 字幕详情"))
info3("保种区-官方保种", "E402 · 三折叠 保种-官方", [("🛡️","官方保种种子","1,234 个 · 全部做种中"),("🎁","免费下载","保种区种子全部免费"),("⏳","保种要求","做种>7天自动移出"),("🎉","免费延续","移出后免费3天")], cols=4)
info3("保种区-全部保种", "E403 · 三折叠 保种-全部", [("🛡️","全部保种","856 个"),("📂","分类","教育/高中/高职/初中/小学"),("🎁","免费下载","全部免费"),("📊","我的保种","12.4TB · 排名12")], cols=4)

# ===== 11. 站务 ×4 =====
info3("关于本站", "E404 · 三折叠 关于", [("🦉","好学PT HxPT","幼小初高学习资源PT站"),("🌱","愿景","好学者如春苗日有所长"),("📅","开站","2025-07-16"),("🎯","定位","7大分类全覆盖")], cols=4)
info3("捐赠支持", "E405 · 三折叠 捐赠", [("💛","捐赠站免池","助力双倍免费"),("🎁","档位","1,000~1,000,000"),("🏆","捐赠榜","gntv·dgvge·kiririn"),("💡","回馈","达200万全局免费")], cols=4)
info3("友情链接", "E406 · 三折叠 友链", [("🔗","好学PT 官方","www.hxpt.org"),("🔗","识典古籍","shidian.com"),("🔗","学习导航","edu.hxpt.org"),("➕","申请友链","联系管理组")], cols=4)
info3("联系我们", "E407 · 三折叠 联系", [("📧","邮箱","admin@hxpt.org"),("💬","PM","站内信联系"),("🎧","客服","10人在线"),("⏰","响应","24h内回复")], cols=4)

# ===== 12. 分类浏览 ×4 =====
for ci, cn in enumerate(["学前教育","职高","教育影音","纪录片"]):
    L(f3_shell(f"浏览-{cn}", f'''
          <div style="background:linear-gradient(135deg,{["#FF8FC7","#8B5CF6","#2FBF9B","#FF7A59"][ci]},{["#FFC93C","#5B6BF5","#2FA8FF","#FFC93C"][ci]});border-radius:14px;padding:15px;color:#fff;margin-bottom:12px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:16px">{cn}</b><span style="background:rgba(255,255,255,.22);border-radius:9px;padding:5px 12px;font-size:10px">{["幼小衔接启蒙资源","职业技能学习资料","纪录片与教育影音","高品质纪录片合集"][ci]}</span></div>
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:10px">{"".join(f'<div style="background:#fff;border-radius:11px;padding:12px;display:flex;gap:9px;align-items:center"><span style="width:34px;height:34px;border-radius:9px;background:{t[2]};color:#fff;display:flex;align-items:center;justify-content:center;font-size:13px">{t[1]}</span><div style="flex:1;min-width:0"><b style="font-size:11px;display:block;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{t[0][:18]}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">{t[5]}做种</p></div></div>' for t in TORRENTS[ci*4:(ci+1)*4])}</div>
          <p style="text-align:center;color:#99a;font-size:11px;margin-top:12px">共 {[1820,1204,3402,2210][ci]} 个种子</p>
''', f"E40{8+ci} · 三折叠 浏览-{cn}"))

# ===== 13. 更多详情 ×6 =====
for ti, t in enumerate(TORRENTS[6:12]):
    L(f3_shell("种子详情", f'''
          <div style="display:grid;grid-template-columns:1.6fr 1fr;gap:14px"><div><div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><div style="display:flex;gap:8px;align-items:flex-start"><b style="font-size:15px;flex:1;line-height:1.5">{t[0]}</b><span class="tag tag-free">Free</span></div><p style="color:#99a;font-size:10px;margin:5px 0 0">发布者 gntv · 发布员 · 2026-09-06</p><div style="display:flex;gap:6px;margin-top:8px;flex-wrap:wrap">{"".join(f'<span style="background:#E8F4FF;color:#2FA8FF;border-radius:8px;padding:2px 8px;font-size:10px">{x}</span>' for x in t[6].split("/"))}</div></div>
          <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:12px;color:#8895aa">基本信息</b><div style="display:grid;grid-template-columns:1fr 1fr;gap:6px;margin-top:8px;font-size:12px"><span style="display:flex;justify-content:space-between;padding:5px 8px;background:#F8FAFC;border-radius:7px"><span style="color:#99a">大小</span><b>{t[4]}</b></span><span style="display:flex;justify-content:space-between;padding:5px 8px;background:#F8FAFC;border-radius:7px"><span style="color:#99a">类型</span><b>{t[3]}</b></span><span style="display:flex;justify-content:space-between;padding:5px 8px;background:#F8FAFC;border-radius:7px"><span style="color:#99a">做种</span><b>{t[5]}</b></span><span style="display:flex;justify-content:space-between;padding:5px 8px;background:#F8FAFC;border-radius:7px"><span style="color:#99a">下载</span><b>3</b></span></div></div></div>
          <div style="display:grid;gap:10px;align-content:start"><div style="background:#F0F7FF;border-radius:12px;padding:14px;text-align:center"><b style="font-size:22px;color:#2FA8FF">{t[5]}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">做种</p></div><button class="f3-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;width:100%;padding:13px;border-radius:11px;font-size:14px">⬇️ 下载种子</button><div style="background:#fff;border-radius:12px;padding:14px;font-size:11px;color:#667;line-height:1.8"><b>📝 简介</b><p style="margin:5px 0 0">{t[0]}，本站官方制作组整理，欢迎下载做种！</p></div></div></div>
''', f"E41{2+ti} · 三折叠 详情-{t[1]}"))

# ===== 14. 半展开 ×6 =====
def f3_half(left_title, left_inner, right_title, right_inner, label):
    aid = next_id()
    return f'''    <dc-artboard id="{aid}-f3h" label="{label}" width="720" height="900">
      <div class="fold3-half">
        <div style="flex:1;background:#F5FAFF;border-radius:12px;overflow:hidden;display:flex;flex-direction:column"><div style="background:#fff;padding:10px 14px;display:flex;justify-content:space-between;align-items:center;border-bottom:1px solid #EEF2F7"><b style="font-size:14px">{left_title}</b><span style="font-size:11px;color:#99a">•••</span></div><div style="padding:12px;flex:1;overflow:hidden">{left_inner}</div></div>
        <div style="width:10px;background:linear-gradient(180deg,#2FA8FF,#5B6BF5);border-radius:5px;margin:0 2px"></div>
        <div style="flex:1;background:#F5FAFF;border-radius:12px;overflow:hidden;display:flex;flex-direction:column"><div style="background:#fff;padding:10px 14px;display:flex;justify-content:space-between;align-items:center;border-bottom:1px solid #EEF2F7"><b style="font-size:14px">{right_title}</b><span style="font-size:11px;color:#99a">详情 ›</span></div><div style="padding:12px;flex:1;overflow:hidden">{right_inner}</div></div>
      </div>
    </dc-artboard>
'''
def hc(title, sub):
    return f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;margin-bottom:6px"><b>{title}</b><p style="color:#99a;font-size:9px;margin:2px 0 0">{sub}</p></div>'
half_pages = [
    ("🎮 游戏", "".join(hc(nm, ds) for ic,nm,ds,_ in GAMES[:5]), "🏆 战绩", "".join(hc(n, v) for n,v in [("五子棋","18胜12负"),("刮刮乐","中奖8次"),("九宫格","大奖×1"),("猜大小","+3,200")])),
    ("🛠️ 工具", "".join(hc(nm, ds) for ic,nm,ds in TOOLS[:5]), "📊 使用统计", "".join(hc(n, v) for n,v in [("图床","今日12张"),("IYUU","1,234辅种"),("压缩","80张")])),
    ("🛡️ 保种区", "".join(hc(nm, f"{sz} · {seeds}做种 · {tm}") for nm,ic,c,cat,sz,seeds,tm in PRESERVE[:4]), "🏅 官种", "".join(hc(nm, f"{sz} · {dt}") for nm,sz,dt in OFFICIALS[:5])),
    ("📋 我的数据", "".join(hc(nm, v) for _,nm,v in MYMENU[:6]), "💰 收益明细", "".join(hc(n, v) for n,v in [("基本","74.2/h"),("勋章","69.2/h"),("官种","325.8/h"),("后宫","93.7/h")])),
    ("🎖️ 勋章墙", "".join(hc(nm, ds) for m,nm,ds in MEDALS[:6]), "🃏 卡牌", "".join(hc(n, v) for n,v in [("N 卡","×8"),("R 卡","×3"),("SR 卡","×1"),("碎片","18/30")])),
    ("📢 公告", "".join(hc(nm, f"{t} · {ds}") for nm,_,t,ds in NEWS[:4]), "📋 事件", "".join(hc(nm, ds) for t,nm,_,ds in EVENTS[:5])),
]
for lt, li, rt, ri, lb in [(a,b,c,d,f"E41{8+i} · 三折叠 半开-{a.split(' ')[-1]}") for i,(a,b,c,d) in enumerate(half_pages)]:
    L(f3_half(lt, li, rt, ri, lb))

# ===== 15. 折叠态 ×6 =====
def f3_closed(title, cards, label):
    aid = next_id()
    cs = "".join(f'<div style="background:rgba(255,255,255,.16);backdrop-filter:blur(10px);border-radius:13px;padding:10px 12px"><div style="display:flex;justify-content:space-between;font-size:11px"><b>{t}</b><span style="opacity:.6">{tm}</span></div><p style="font-size:11px;opacity:.9;margin:4px 0 0;line-height:1.6">{tx}</p></div>' for t, tx, tm in cards)
    return f'''    <dc-artboard id="{aid}-f3c" label="{label}" width="340" height="720">
      <div class="fold3-closed">
        <div style="position:absolute;inset:0;background:linear-gradient(160deg,#1E3A5F,#0F172A 70%)"></div>
        <div style="position:relative;height:100%;display:flex;flex-direction:column;align-items:center;padding:18px 14px;color:#fff">
          <div style="align-self:flex-start;font-size:12px;opacity:.8">9:41</div>
          <div style="margin-top:20px;width:50px;height:50px;border-radius:14px;background:rgba(255,255,255,.16);display:flex;align-items:center;justify-content:center;font-size:24px">🦉</div>
          <div style="margin-top:10px;text-align:center"><b style="font-size:15px">{title}</b><p style="font-size:10px;opacity:.7;margin:3px 0 0">好学 HxPT</p></div>
          <div style="margin-top:18px;width:100%;display:grid;gap:8px">{cs}</div>
          <div style="margin-top:auto;font-size:10px;opacity:.6">好学PT · 种子⇄成长</div>
        </div>
      </div>
    </dc-artboard>
'''
closed_pages = [
    ("双倍免费预告", [("🎉 站免池","进度 52.3%，达 200 万下月双倍免费","实时"),("🔥 中秋活动","9月15-17日全站免费","09-15")]),
    ("学习计划", [("📚 今日学习","语文 45min · 数学 30min","今日"),("✅ 目标完成","3/5 项 · 继续加油","实时")]),
    ("勋章进度", [("🎖️ 节气勋章","立春/雨水/惊蛰 已收集","实时"),("🏆 学习课堂","3/6 枚 · 还差一半","实时")]),
    ("保种提醒", [("🛡️ 保种到期","3 个种子将于明天移出保种区","提醒"),("📊 当前保种","12.4 TB · 排名 12","实时")]),
    ("收益日报", [("💰 今日收益","+13,507 火花 · 高于昨日","日报"),("🏦 银行结息","活期 +4.69 火花","00:00")]),
    ("论坛动态", [("💬 新回复","alan5914 回复了你的帖子","10:15", ), ("⭐ 新收藏","你的资源被 3 人收藏","09:50")]),
]
for i, (cp_title, cards) in enumerate(closed_pages):
    L(f3_closed(cp_title, cards, f"E42{4+i} · 三折叠 折叠态-{cp_title}"))

# ===== 16. 分屏 ×4 =====
def pane(title, inner):
    return f'<div style="flex:1;background:#F5FAFF;display:flex;flex-direction:column;overflow:hidden"><div style="background:#fff;padding:9px 12px;font-size:12px;border-bottom:1px solid #EEF2F7;display:flex;justify-content:space-between"><b>{title}</b><span style="color:#99a;font-size:10px">•••</span></div><div style="padding:10px;flex:1;overflow:hidden;display:grid;gap:7px">{inner}</div></div>'
split_pages = [
    ("我的+任务+勋章", ["<div style='background:#fff;border-radius:8px;padding:9px 10px;font-size:11px;display:flex;justify-content:space-between'><b>上传</b><b style='color:#2FA8FF'>108.97T</b></div>","<div style='background:#fff;border-radius:8px;padding:9px 10px;font-size:11px;display:flex;justify-content:space-between'><b>分享率</b><b>38.5</b></div>","<div style='background:#fff;border-radius:8px;padding:9px 10px;font-size:11px;display:flex;justify-content:space-between'><b>火花</b><b style='color:#FF7A59'>85.8M</b></div>"], ["<div style='background:#fff;border-radius:8px;padding:9px 10px;font-size:11px;display:flex;justify-content:space-between'><b>上传10种子</b><span style='color:#2FBF9B'>✓</span></div>","<div style='background:#fff;border-radius:8px;padding:9px 10px;font-size:11px;display:flex;justify-content:space-between'><b>下载5种子</b><span style='color:#FF7A59'>3/5</span></div>"], ["<div style='background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px'><span style='font-size:18px'>🏅</span><b style='display:block'>新人</b></div>","<div style='background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px'><span style='font-size:18px'>📚</span><b style='display:block'>初出</b></div>"]),
    ("论坛+课本+勋章", ["<div style='background:#fff;border-radius:8px;padding:9px 10px;font-size:11px'><b>小学部交流</b><p style='color:#99a;font-size:9px;margin:3px 0 0'>56帖 · 最新10:20</p></div>","<div style='background:#fff;border-radius:8px;padding:9px 10px;font-size:11px'><b>初中部交流</b><p style='color:#99a;font-size:9px;margin:3px 0 0'>48帖 · 最新09:45</p></div>"], ["<div style='background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px'><span style='font-size:18px'>📘</span><b style='display:block'>语文</b></div>","<div style='background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px'><span style='font-size:18px'>📗</span><b style='display:block'>数学</b></div>"], ["<div style='background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px'><span style='font-size:18px'>🌸</span><b style='display:block'>立春</b></div>","<div style='background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px'><span style='font-size:18px'>🏆</span><b style='display:block'>学霸</b></div>"]),
    ("保种+官种+免费", ["<div style='background:#fff;border-radius:8px;padding:9px 10px;font-size:11px'><b>官方保种</b><p style='color:#99a;font-size:9px;margin:3px 0 0'>1,234个 · 全做种</p></div>","<div style='background:#fff;border-radius:8px;padding:9px 10px;font-size:11px'><b>全部保种</b><p style='color:#99a;font-size:9px;margin:3px 0 0'>856个</p></div>"], ["<div style='background:#fff;border-radius:8px;padding:9px 10px;font-size:11px'><b>官种加成</b><p style='color:#FF7A59;font-size:9px;margin:3px 0 0'>5x 火花收益</p></div>"], ["<div style='background:#fff;border-radius:8px;padding:9px 10px;font-size:11px'><b>免费种子</b><p style='color:#99a;font-size:9px;margin:3px 0 0'>1,203个</p></div>"]),
    ("银行+商店+农场", ["<div style='background:#fff;border-radius:8px;padding:9px 10px;font-size:11px'><b>总资产</b><p style='font-size:13px;margin:3px 0 0'>85,814,940</p></div>","<div style='background:#fff;border-radius:8px;padding:9px 10px;font-size:11px;display:flex;justify-content:space-between'><b>活期</b><b>3,321</b></div>"], ["<div style='background:#fff;border-radius:8px;padding:9px 10px;font-size:11px;display:flex;justify-content:space-between'><b>1GB上传</b><b style='color:#FF7A59'>400</b></div>","<div style='background:#fff;border-radius:8px;padding:9px 10px;font-size:11px;display:flex;justify-content:space-between'><b>补签卡</b><b style='color:#FF7A59'>1,000</b></div>"], ["<div style='background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px'><span style='font-size:18px'>🌾</span><b style='display:block'>小麦</b></div>","<div style='background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px'><span style='font-size:18px'>🐔</span><b style='display:block'>鸡</b></div>"]),
]
for i, (sp_title, p1, p2, p3) in enumerate(split_pages):
    titles = sp_title.split("+") + ["面板3"]
    cols_html = pane(titles[0], "".join(p1)) + pane(titles[1], "".join(p2)) + pane(titles[2], "".join(p3))
    L(f'''    <dc-artboard id="{next_id()}-f3s" label="E43{0+i} · 三折叠 分屏-{sp_title}" width="1000" height="760">
      <div class="fold3-screen" style="flex-direction:row;overflow:hidden;border:none">{cols_html}</div>
    </dc-artboard>
''')

html = "\n".join(out)
open('/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/gen/fold3b_boards.html','w',encoding='utf-8').write(html)
print("fold3b boards:", len(out), "last id:", n[0])

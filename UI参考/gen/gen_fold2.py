# -*- coding: utf-8 -*-
"""生成 双折叠 内屏(720x900) 60页 + 外屏(340x720) 15页"""
import sys
sys.path.insert(0, '/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/gen')
from data import *

n = [50]
def next_id(prefix='e'):
    n[0] += 1
    return f"{prefix}{n[0]}"

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

def f2_card_row(icon, bg, name, sub, right):
    return f'<div style="display:flex;gap:9px;align-items:center;background:#fff;border-radius:11px;padding:11px 12px"><span style="width:34px;height:34px;border-radius:9px;background:{bg};color:#fff;display:flex;align-items:center;justify-content:center;font-size:13px;flex-shrink:0">{icon}</span><div style="flex:1;min-width:0"><b style="font-size:12px;display:block;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{name}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">{sub}</p></div><div style="text-align:right;flex-shrink:0">{right}</div></div>'

def tag(t, c="#2FA8FF", bg="#E8F4FF"):
    return f'<span style="background:{bg};color:{c};border-radius:8px;padding:1px 6px;font-size:9px;flex-shrink:0">{t}</span>'

def chip(t, on=False):
    return f'<span style="background:{"#2FA8FF" if on else "#F0F4F8"};color:{"#fff" if on else "#667"};border-radius:14px;padding:5px 12px;font-size:11px">{t}</span>'

out = []
L = out.append

# ===== 1. 首页工作台 =====
L(f2_shell("首页", f'''
          <div style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);border-radius:14px;padding:16px;color:#fff;margin-bottom:12px">
            <h2 style="margin:0;font-size:17px">好学者如春苗，日有所长 🌱</h2>
            <p style="font-size:11px;opacity:.9;margin:4px 0 0">19,434 个学习资源 · 4,645 位同学一起学习</p>
            <div style="display:flex;gap:8px;margin-top:10px"><span style="background:rgba(255,255,255,.2);border-radius:10px;padding:6px 14px;font-size:12px">浏览资源</span><span style="background:rgba(255,255,255,.2);border-radius:10px;padding:6px 14px;font-size:12px">发布资源</span></div>
          </div>
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin-bottom:12px">
            <div style="background:#fff;border-radius:10px;padding:10px;text-align:center"><b style="font-size:14px">108.97T</b><p style="color:#99a;font-size:10px;margin:2px 0 0">上传</p></div>
            <div style="background:#fff;border-radius:10px;padding:10px;text-align:center"><b style="font-size:14px">38.5</b><p style="color:#99a;font-size:10px;margin:2px 0 0">分享率</p></div>
            <div style="background:#fff;border-radius:10px;padding:10px;text-align:center"><b style="font-size:14px">85.8M</b><p style="color:#99a;font-size:10px;margin:2px 0 0">火花</p></div>
          </div>
          <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:8px"><b style="font-size:13px">📢 最新公告</b><span style="color:#99a;font-size:10px">查看全部 ›</span></div>
          <div style="display:grid;gap:8px;margin-bottom:12px">
            <div style="background:#fff;border-radius:10px;padding:10px 12px"><div style="display:flex;justify-content:space-between;font-size:12px"><b>站免池月度进度通报（9月）</b><span style="color:#99a;font-size:10px">09-07</span></div><p style="color:#667;font-size:11px;margin:3px 0 0">当前进度 52.3%，继续加油冲 200 万！</p></div>
            <div style="background:#fff;border-radius:10px;padding:10px 12px"><div style="display:flex;justify-content:space-between;font-size:12px"><b>中秋节双倍免费活动</b><span style="color:#99a;font-size:10px">09-15</span></div><p style="color:#667;font-size:11px;margin:3px 0 0">9月15-17日全站免费</p></div>
          </div>
          <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:8px"><b style="font-size:13px">🔥 最新资源</b><span style="color:#99a;font-size:10px">更多 ›</span></div>
          <div style="display:grid;gap:8px">{"".join(f2_card_row(t[1], t[2], t[0][:26]+"…" if len(t[0])>26 else t[0], f"{t[3]} · {t[4]} · {t[5]}做种", tag(t[6].split('/')[0], bg="#E8F8F2" if t[6].startswith('Free') else "#FFF4E0")) for t in TORRENTS[:5])}</div>
''', "E51 · 双折叠 首页工作台"))

# ===== 2. 种子浏览 (分类×4) =====
cats = [("全部", "全部资源"), ("学前教育", "幼"), ("小学", "小"), ("初中", "初"), ("职高", "职"), ("高中", "高"), ("教育影音", "影"), ("纪录片", "纪")]
for ci, (cn, _) in enumerate([("全部",0),("小学",0),("初中",0),("高中",0)]):
    L(f2_shell("资源浏览", f'''
          <div style="display:flex;gap:6px;margin-bottom:10px;flex-wrap:wrap">{"".join(chip(c, on=(c==cn)) for c,_ in cats)}</div>
          <div style="display:grid;gap:9px">{"".join(f2_card_row(t[1], t[2], t[0], f"{t[3]} · {t[4]} · {t[5]}做种 {t[6]}做种", f'<span style="color:#99a;font-size:10px">{t[5]}↓</span><br>{tag(t[6].split("/")[0], bg="#E8F8F2" if t[6].startswith("Free") else "#FFF4E0")}') for t in TORRENTS[ci*4:(ci+1)*4])}</div>
          <div style="padding:12px;text-align:center;color:#99a;font-size:11px">第 1 / 486 页 · 共 19,434 个种子</div>
''', f"E5{2+ci} · 双折叠 浏览-{cn}"))

# ===== 3. 种子详情 (×6) =====
for ti, t in enumerate(TORRENTS[:6]):
    L(f2_shell("种子详情", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px">
            <div style="display:flex;gap:8px;align-items:flex-start"><b style="font-size:14px;flex:1;line-height:1.5">{t[0]}</b><span class="tag tag-free">Free</span></div>
            <p style="color:#99a;font-size:10px;margin:6px 0 0">发布者 gntv · 发布员 · 2026-09-06</p>
            <div style="display:flex;gap:6px;margin-top:8px;flex-wrap:wrap">{"".join(tag(x) for x in t[6].split("/"))}</div>
          </div>
          <div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:10px"><b style="font-size:12px;color:#8895aa">基本信息</b><div style="display:grid;gap:6px;margin-top:8px;font-size:12px"><span style="display:flex;justify-content:space-between"><span style="color:#99a">大小</span><b>{t[4]}</b></span><span style="display:flex;justify-content:space-between"><span style="color:#99a">类型</span><b>{t[3]}</b></span><span style="display:flex;justify-content:space-between"><span style="color:#99a">媒介</span><b>书籍/视频</b></span><span style="display:flex;justify-content:space-between"><span style="color:#99a">发布年份</span><b>2026</b></span></div></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:10px">
            <div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:16px;color:#2FA8FF">{t[5]}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">做种</p></div>
            <div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:16px;color:#2FBF9B">{t[6].split(" ")[-1] if False else t[5]}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">下载</p></div>
          </div>
          <button class="f2-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px;margin-bottom:8px">⬇️ 下载种子</button>
          <div style="background:#fff;border-radius:10px;padding:11px;font-size:12px;color:#667"><b>📎 种子链接：</b><span style="color:#2FA8FF">https://www.hxpt.org/download.php?id=19605</span></div>
''', f"E5{6+ti} · 双折叠 详情-{t[1]}"))

# ===== 4. 论坛版块 (×4) =====
for fi, (fn, subs, c) in enumerate(FORUMS):
    L(f2_shell("论坛", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:14px;color:{c}">{fn}</b><div style="display:grid;gap:8px;margin-top:10px">{"".join(f'<div style="display:flex;justify-content:space-between;align-items:center;border:1px solid #EEF2F7;border-radius:10px;padding:10px 12px;font-size:12px"><b>{s}</b><span style="color:#99a">{cnt}</span></div>' for s, cnt in subs)}</div></div>
          <div style="background:#fff;border-radius:12px;padding:12px"><b style="font-size:12px;color:#8895aa">📌 最新主题</b><div style="display:grid;gap:6px;margin-top:8px;font-size:11px"><div style="display:flex;justify-content:space-between"><span>IYUU辅种保姆级教程</span><span style="color:#99a">gntv</span></div><div style="display:flex;justify-content:space-between"><span>新人必读：站点规则</span><span style="color:#99a">管理组</span></div></div></div>
''', f"E7{0+fi} · 双折叠 论坛-{fn.replace('📢 ','').replace('📚 ','').replace('🎁 ','').replace('💻 ','')}"))

# ===== 5. 课本中心 (×4) =====
sub_groups = [("小学", SUBJECTS[:6]), ("初中", SUBJECTS[2:10]), ("高中", SUBJECTS[6:14]), ("全学段", SUBJECTS[14:])]
for gi, (gn, subs) in enumerate(sub_groups):
    L(f2_shell("课本中心", f'''
          <div style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);border-radius:14px;padding:14px;color:#fff;margin-bottom:10px"><b style="font-size:15px">📚 课本中心 · {gn}</b><p style="font-size:11px;opacity:.9;margin:4px 0 0">小学、初中、高中课本资源一键下载</p><div style="display:flex;gap:6px;margin-top:8px"><span style="background:rgba(255,255,255,.2);border-radius:8px;padding:4px 10px;font-size:10px">📋 我的提交</span><span style="background:rgba(255,255,255,.2);border-radius:8px;padding:4px 10px;font-size:10px">➕ 创建课本</span></div></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px">{"".join(f'<div style="background:#fff;border-radius:10px;padding:11px;text-align:center"><span style="font-size:20px">📘</span><b style="display:block;font-size:12px;margin-top:2px">{s}</b><span style="color:#99a;font-size:10px">人教版 · {10+gi*7}本</span></div>' for s in subs)}</div>
''', f"E7{4+gi} · 双折叠 课本-{gn}"))

# ===== 6. 勋章墙 =====
L(f2_shell("勋章墙", f'''
          <div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:10px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:13px">我的收藏</b><p style="color:#99a;font-size:11px;margin:2px 0 0">已收集 30 / 81 · 37%</p></div><div style="width:90px;height:8px;background:#F0F4F8;border-radius:5px;overflow:hidden"><div style="width:37%;height:100%;background:linear-gradient(90deg,#2FA8FF,#FFC93C);border-radius:5px"></div></div></div>
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px">{"".join(f'<div style="background:#fff;border-radius:11px;padding:11px;text-align:center;{"border:2px solid #E8F4FF" if i<4 else "opacity:.45"}"><span style="font-size:24px">{m}</span><b style="display:block;font-size:11px;margin-top:3px">{nm}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">{ds}</p></div>' for i,(m,nm,ds) in enumerate(MEDALS))}</div>
          <div style="display:flex;gap:6px;margin-top:10px;flex-wrap:wrap;font-size:10px"><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:4px 9px">二十四节气</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:4px 9px">学习课堂</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:4px 9px">节日系列</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:4px 9px">工作组</span></div>
''', "E79 · 双折叠 勋章墙"))

# ===== 7. 排行榜 (×4) =====
rank_types = [("上传者", "上传量"), ("下载者", "下载量"), ("做种", "做种数"), ("分享率", "分享率")]
for ri, (rn, col) in enumerate(rank_types):
    rows = "".join(f'<div style="display:flex;align-items:center;gap:9px;padding:10px 12px;background:{"linear-gradient(135deg,#FFF8E8,#FFF4D6)" if i==0 else "#fff"};border-radius:11px"><span style="width:20px;text-align:center;font-weight:700;color:{"#FFC93C" if i<3 else "#99a"}">{i+1}</span><img src="{OWL}" style="width:30px;height:30px;border-radius:50%"><div style="flex:1"><b style="font-size:12px">{u}</b><p style="color:#99a;font-size:9px;margin:0">{cl}</p></div><b style="font-size:12px;color:{"#FF7A59" if i==0 else "#1F2A44"}">{v}</b></div>' for i,(u,cl,v,_) in enumerate(RANKS))
    L(f2_shell("排行榜", f'''
          <div style="display:flex;gap:6px;margin-bottom:10px;flex-wrap:wrap">{"".join(chip(x, on=(x==rn)) for x in ["上传者","下载者","做种","分享率"])}</div>
          <div style="display:grid;gap:8px">{rows}</div>
          <div style="display:flex;gap:6px;margin-top:10px;font-size:10px"><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:4px 10px">范围 Top 21</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:4px 10px">排版 卡片</span></div>
''', f"E8{0+ri} · 双折叠 排行-{rn}"))

# ===== 8. 站免池 =====
L(f2_shell("站免池", f'''
          <div style="background:linear-gradient(135deg,#FF7A59,#FFC93C);border-radius:14px;padding:16px;color:#fff;margin-bottom:10px"><p style="font-size:11px;opacity:.9;margin:0">🎉 站免池 · 大家一起冲</p><b style="font-size:20px">52.3%</b><div style="height:9px;background:rgba(255,255,255,.3);border-radius:5px;overflow:hidden;margin-top:6px"><div style="width:52.3%;height:100%;background:#fff;border-radius:5px"></div></div><p style="font-size:10px;opacity:.9;margin:6px 0 0">1,046,000 / 2,000,000 火花 · 达 200 万下月全局双倍免费</p></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:10px">{"".join(f'<div style="background:#fff;border-radius:10px;padding:11px;text-align:center"><b style="font-size:14px;color:#FF7A59">{t}</b><p style="color:#99a;font-size:9px;margin:2px 0 0">火花</p></div>' for t in POOL_TIERS[:6])}</div>
          <button class="f2-btn" style="background:#FF7A59;color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px">💛 捐赠火花，助力全局免费</button>
          <div style="background:#FFF8E8;border-radius:10px;padding:10px;margin-top:10px;font-size:11px;color:#667">📜 最近捐赠：gntv +1,000,000 · dgvge +500,000 · kiririn +100,000</div>
''', "E84 · 双折叠 站免池"))

# ===== 9. 银行 (×3) =====
bank_tabs = [("总览", 0), ("定期存款", 1), ("贷款", 2)]
for bi, (bt, _) in enumerate(bank_tabs):
    if bt == "总览":
        body = f'''<div style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);border-radius:14px;padding:16px;color:#fff;margin-bottom:10px"><p style="font-size:11px;opacity:.9;margin:0">总资产</p><b style="font-size:22px">85,814,940.71</b><p style="font-size:10px;opacity:.9;margin:4px 0 0">时魔 +469.14/h · 最大可贷 46,924.38</p></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:10px"><div style="background:#fff;border-radius:10px;padding:11px"><p style="color:#99a;font-size:10px;margin:0">活期余额</p><b style="font-size:15px">3,321.81</b></div><div style="background:#fff;border-radius:10px;padding:11px"><p style="color:#99a;font-size:10px;margin:0">在投定期</p><b style="font-size:15px">0</b></div><div style="background:#fff;border-radius:10px;padding:11px"><p style="color:#99a;font-size:10px;margin:0">贷款负债</p><b style="font-size:15px">0</b></div><div style="background:#fff;border-radius:10px;padding:11px"><p style="color:#99a;font-size:10px;margin:0">站内余额</p><b style="font-size:15px">85,811,618.9</b></div></div>
          <div style="background:#fff;border-radius:10px;padding:11px"><b style="font-size:12px;color:#8895aa">📌 利率表</b>{"".join(f'<div style="display:flex;justify-content:space-between;font-size:11px;padding:5px 0;border-bottom:1px solid #F0F4F8"><span>{r}</span><b style="color:{c}">{v}</b></div>' for r,v,c in RATES)}</div>'''
    elif bt == "定期存款":
        body = f'''<div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:13px">📈 选择定期期限</b><div style="display:grid;gap:8px;margin-top:10px">{"".join(f'<div style="display:flex;justify-content:space-between;align-items:center;border:1px solid #EEF2F7;border-radius:10px;padding:10px 12px;font-size:12px"><span>{r}</span><b style="color:#2FA8FF">{v}</b><span style="background:#E8F4FF;color:#2FA8FF;border-radius:8px;padding:3px 8px;font-size:10px">存入</span></div>' for r,v,_ in RATES[1:])}</div></div>
          <div style="background:#FFF8E8;border-radius:10px;padding:10px;font-size:11px;color:#667">💡 最小存款 10,000 火花 · 提前支取手续费 1%</div>'''
    else:
        body = f'''<div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:13px">🏦 申请贷款</b><div style="display:grid;gap:8px;margin-top:10px"><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">贷款金额</label><input class="gi" placeholder="最大可贷 46,924.38 火花" style="width:100%;height:36px;font-size:12px"></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">期限</label><div style="display:flex;gap:6px;flex-wrap:wrap">{"".join(chip(r.replace(" 定期",""), on=(i==1)) for i,(r,v,c) in enumerate(RATES[1:]))}</div></div></div><button class="f2-btn" style="background:#FF7A59;color:#fff;border:none;width:100%;margin-top:10px;padding:11px;border-radius:10px;font-size:13px">确认贷款</button></div>
          <div style="background:#FFF8E8;border-radius:10px;padding:10px;font-size:11px;color:#667">💡 贷款利息 7天0.08% / 30天0.12% / 90天0.18% / 180天0.2% / 365天0.22%</div>'''
    L(f2_shell("火花银行", f'<div style="display:flex;gap:6px;margin-bottom:10px">{"".join(chip(x, on=(x==bt)) for x in ["总览","定期存款","贷款"])}</div>{body}', f"E8{5+bi} · 双折叠 银行-{bt}"))

# ===== 10. 农场 (×2) =====
L(f2_shell("好学农场", f'''
          <div style="display:flex;gap:6px;margin-bottom:10px"><span style="background:#2FBF9B;color:#fff;border-radius:16px;padding:5px 12px;font-size:12px">🌾 农作物</span><span style="background:#F0F4F8;color:#667;border-radius:16px;padding:5px 12px;font-size:12px">🐔 动物</span><span style="background:#F0F4F8;color:#667;border-radius:16px;padding:5px 12px;font-size:12px">🥬 菜市场</span></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px">{"".join(f'<div style="background:#fff;border-radius:10px;padding:11px;text-align:center;{"border:2px solid #FFF4E0" if i==0 else ""}"><span style="font-size:22px">{ic}</span><b style="display:block;font-size:12px;margin-top:2px">{nm}</b><p style="color:#99a;font-size:10px;margin:2px 0">{"生长中 2h" if i==0 else "空闲"}</p><b style="color:#FF7A59;font-size:11px">{v} 火花</b></div>' for i,(ic,nm,tm,v) in enumerate(CROPS))}</div>
          <div style="background:#FFF8E8;border-radius:10px;padding:10px;margin-top:10px;font-size:11px;color:#667"><b>💡 提示：</b>有效期 5 天，20% 概率双倍收获；菜市场每日 0/4/8/12/16/20 点刷新 ±50%</div>
''', "E88 · 双折叠 农场-农作物"))
L(f2_shell("好学农场", f'''
          <div style="display:flex;gap:6px;margin-bottom:10px"><span style="background:#F0F4F8;color:#667;border-radius:16px;padding:5px 12px;font-size:12px">🌾 农作物</span><span style="background:#2FBF9B;color:#fff;border-radius:16px;padding:5px 12px;font-size:12px">🐔 动物</span><span style="background:#F0F4F8;color:#667;border-radius:16px;padding:5px 12px;font-size:12px">🥬 菜市场</span></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px">{"".join(f'<div style="background:#fff;border-radius:10px;padding:11px;text-align:center;{"border:2px solid #E8F8F2" if i==0 else ""}"><span style="font-size:22px">{ic}</span><b style="display:block;font-size:12px;margin-top:2px">{nm}</b><p style="color:#99a;font-size:10px;margin:2px 0">{"已成熟 🎉" if i==0 else "空闲"}</p><b style="color:#FF7A59;font-size:11px">{v} 火花</b></div>' for i,(ic,nm,tm,v) in enumerate(ANIMALS))}</div>
          <div style="background:#FFF8E8;border-radius:10px;padding:10px;margin-top:10px;font-size:11px;color:#667">💡 动物收获后可前往菜市场出售，价格每日刷新</div>
''', "E89 · 双折叠 农场-动物"))

# ===== 11. 菜市场 =====
L(f2_shell("菜市场", f'''
          <div style="background:linear-gradient(135deg,#2FBF9B,#FFC93C);border-radius:14px;padding:14px;color:#fff;margin-bottom:10px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:14px">🥬 今日菜市场</b><p style="font-size:10px;opacity:.9;margin:2px 0 0">下次刷新 12:00 · 价格波动 ±50%</p></div><span style="background:rgba(255,255,255,.25);border-radius:10px;padding:5px 12px;font-size:11px">🔄 刷新</span></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px">
            <div style="background:#fff;border-radius:10px;padding:11px;text-align:center"><span style="font-size:20px">🌾</span><b style="display:block;font-size:12px;margin-top:2px">小麦</b><p style="color:#99a;font-size:10px;margin:2px 0">收购价</p><b style="font-size:14px;color:#2FBF9B">620 🔥</b></div>
            <div style="background:#fff;border-radius:10px;padding:11px;text-align:center"><span style="font-size:20px">🌽</span><b style="display:block;font-size:12px;margin-top:2px">玉米</b><p style="color:#99a;font-size:10px;margin:2px 0">收购价</p><b style="font-size:14px;color:#FF7A59">880</b></div>
            <div style="background:#fff;border-radius:10px;padding:11px;text-align:center"><span style="font-size:20px">🥜</span><b style="display:block;font-size:12px;margin-top:2px">花生</b><p style="color:#99a;font-size:10px;margin:2px 0">收购价</p><b style="font-size:14px;color:#FF7A59">1,420</b></div>
            <div style="background:#fff;border-radius:10px;padding:11px;text-align:center"><span style="font-size:20px">🥔</span><b style="display:block;font-size:12px;margin-top:2px">土豆</b><p style="color:#99a;font-size:10px;margin:2px 0">收购价</p><b style="font-size:14px;color:#FF7A59">1,980</b></div>
          </div>
          <div style="background:#E8F8F2;border-radius:10px;padding:10px;margin-top:10px;font-size:11px;color:#2FBF9B">📈 小麦今日高价 620（+24%），趁早出售！</div>
''', "E90 · 双折叠 菜市场"))

# ===== 12. 火花商店 (×3) =====
shop_tabs = [("全部", SHOP), ("流量", SHOP[:6]), ("个性", SHOP[6:])]
for si, (st, items) in enumerate(shop_tabs):
    L(f2_shell("火花商店", f'''
          <div style="display:flex;gap:6px;margin-bottom:10px">{"".join(chip(x, on=(x==st)) for x in ["全部","流量","个性"])}</div>
          <div style="display:grid;gap:8px">{"".join(f'<div style="display:flex;justify-content:space-between;align-items:center;background:#fff;border-radius:10px;padding:10px 12px;font-size:12px"><div style="display:flex;align-items:center;gap:8px"><span style="width:30px;height:30px;border-radius:8px;background:{"#E8F4FF" if "上传" in nm or "下载" in nm else "#FFE8F5"};display:flex;align-items:center;justify-content:center;font-size:13px">{"📤" if "上传" in nm else "📥" if "下载" in nm else "🎁"}</span><b>{nm}</b></div><div style="text-align:right"><b style="color:#FF7A59;font-size:12px">{pr}</b><p style="color:#99a;font-size:9px;margin:0">{cat}</p></div></div>' for nm, pr, cat in items)}</div>
''', f"E9{1+si} · 双折叠 商店-{st}"))

# ===== 13. 任务中心 =====
L(f2_shell("任务中心", f'''
          <div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:10px"><b style="font-size:13px">🏆 我的任务</b><div style="display:grid;gap:8px;margin-top:8px">{"".join(f'<div style="display:flex;gap:9px;align-items:center;border:1px solid #EEF2F7;border-radius:10px;padding:9px 11px;font-size:11px"><span style="font-size:18px">{ic}</span><div style="flex:1"><b style="font-size:12px">{nm}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">奖励 {rw}</p></div><span style="background:{c}20;color:{c};border-radius:10px;padding:3px 8px;font-size:9px">{st}</span></div>' for ic,nm,rw,st,c in TASKS)}</div></div>
          <div style="background:linear-gradient(135deg,#E8F4FF,#E8F0FF);border-radius:12px;padding:12px"><b style="font-size:13px">📋 绩效考核（保种员 5T 版）</b><p style="color:#99a;font-size:10px;margin:3px 0 0">月领 200,000 火花</p><div style="display:flex;gap:10px;margin-top:8px"><div style="flex:1;background:#fff;border-radius:9px;padding:8px;text-align:center"><b style="font-size:14px;color:#2FA8FF">28/30</b><p style="color:#99a;font-size:9px;margin:1px 0 0">操作总数</p></div><div style="flex:1;background:#fff;border-radius:9px;padding:8px;text-align:center"><b style="font-size:14px;color:#2FA8FF">25/30</b><p style="color:#99a;font-size:9px;margin:1px 0 0">通过审核</p></div></div></div>
''', "E94 · 双折叠 任务中心"))

# ===== 14. 签到 =====
L(f2_shell("签到", f'''
          <div style="background:linear-gradient(135deg,#2FBF9B,#2FA8FF);border-radius:14px;padding:16px;color:#fff;margin-bottom:10px;display:flex;justify-content:space-between;align-items:center"><div><p style="font-size:10px;opacity:.9;margin:0">已连续签到</p><b style="font-size:24px">120 天</b><p style="font-size:10px;opacity:.9;margin:4px 0 0">累计获得 42,000 火花</p></div><div style="background:rgba(255,255,255,.2);border-radius:14px;padding:10px 14px;text-align:center"><span style="font-size:20px">✅</span><p style="font-size:9px;margin:2px 0 0">今日已签</p></div></div>
          <div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:10px"><b style="font-size:12px;color:#8895aa">📅 本月签到</b><div style="display:grid;grid-template-columns:repeat(7,1fr);gap:5px;margin-top:8px;text-align:center;font-size:10px"><span style="color:#99a">一</span><span style="color:#99a">二</span><span style="color:#99a">三</span><span style="color:#99a">四</span><span style="color:#99a">五</span><span style="color:#99a">六</span><span style="color:#99a">日</span><span></span><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">1</div><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">2</div><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">3</div><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">4</div><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">5</div><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">6</div><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">7</div><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">8</div><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">9</div><div style="background:#2FBF9B;color:#fff;border-radius:7px;padding:3px 0;font-weight:700">10</div></div></div>
          <div style="background:#FFF8E8;border-radius:10px;padding:10px;font-size:11px;color:#667"><b>🎁 奖励：</b>10天200 / 20天500 / 30天1000 / 60天3000 / 100天10000+勋章 / 365天50000+年限勋章</div>
''', "E95 · 双折叠 签到"))

# ===== 15. 消息 =====
L(f2_shell("消息", f'''
          <div style="display:flex;gap:6px;margin-bottom:10px"><span style="background:#2FA8FF;color:#fff;border-radius:16px;padding:5px 12px;font-size:12px">收件箱(5)</span><span style="background:#F0F4F8;color:#667;border-radius:16px;padding:5px 12px;font-size:12px">发件箱</span><span style="background:#F0F4F8;color:#667;border-radius:16px;padding:5px 12px;font-size:12px">系统</span></div>
          <div style="display:grid;gap:8px">{"".join(f'<div style="background:#fff;border-radius:10px;padding:11px 12px"><div style="display:flex;justify-content:space-between;font-size:11px"><b>{f}</b><span style="color:#99a">{t}</span></div><p style="color:#667;font-size:11px;margin:4px 0 0;line-height:1.6">{m}</p></div>' for f,m,t in MSGS)}</div>
''', "E96 · 双折叠 消息中心"))

# ===== 16. 我的数据菜单 =====
L(f2_shell("我的数据", f'''
          <div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:10px;display:flex;align-items:center;gap:10px"><img src="{OWL}" style="width:44px;height:44px;border-radius:50%"><div><b style="font-size:14px">gntv</b><p style="color:#99a;font-size:10px;margin:2px 0 0">发布员 · 加入 2025-07-16</p></div><div style="flex:1;text-align:right"><span style="background:#FFE8F5;color:#FF8FC7;border-radius:10px;padding:3px 10px;font-size:10px">🎖️ 勋章 30/81</span></div></div>
          <div style="display:grid;gap:8px">{"".join(f2_card_row(ic, "#E8F4FF", nm, "点击查看详情", f'<span style="color:#99a;font-size:10px">{v}</span> ›') for ic,nm,v in MYMENU)}</div>
''', "E97 · 双折叠 我的数据"))

# ===== 17. 火花收益 =====
L(f2_shell("我的火花收益", f'''
          <div style="background:linear-gradient(135deg,#FFC93C,#FF7A59);border-radius:14px;padding:16px;color:#fff;margin-bottom:10px"><p style="font-size:10px;opacity:.9;margin:0">当前火花</p><b style="font-size:22px">85,815,181.6</b><p style="font-size:10px;opacity:.9;margin:4px 0 0">每小时收益 562.845</p></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:10px"><div style="background:#fff;border-radius:10px;padding:10px"><p style="color:#99a;font-size:10px;margin:0">基本奖励</p><b style="font-size:14px">74.2</b></div><div style="background:#fff;border-radius:10px;padding:10px"><p style="color:#99a;font-size:10px;margin:0">勋章加成</p><b style="font-size:14px">69.215</b><p style="color:#99a;font-size:9px;margin:0">1.03x</p></div><div style="background:#fff;border-radius:10px;padding:10px"><p style="color:#99a;font-size:10px;margin:0">官种加成</p><b style="font-size:14px">325.750</b><p style="color:#99a;font-size:9px;margin:0">5x</p></div><div style="background:#fff;border-radius:10px;padding:10px"><p style="color:#99a;font-size:10px;margin:0">后宫加成</p><b style="font-size:14px">93.680</b><p style="color:#99a;font-size:9px;margin:0">0.1x</p></div></div>
          <div style="background:#fff;border-radius:10px;padding:11px"><b style="font-size:12px;color:#8895aa">📈 近 7 天趋势</b><div style="display:flex;align-items:flex-end;gap:7px;height:64px;margin-top:8px"><div style="flex:1;background:#E8F4FF;height:30%;border-radius:3px 3px 0 0"></div><div style="flex:1;background:#E8F4FF;height:45%;border-radius:3px 3px 0 0"></div><div style="flex:1;background:#E8F4FF;height:40%;border-radius:3px 3px 0 0"></div><div style="flex:1;background:#2FA8FF;height:60%;border-radius:3px 3px 0 0"></div><div style="flex:1;background:#2FA8FF;height:55%;border-radius:3px 3px 0 0"></div><div style="flex:1;background:#2FA8FF;height:70%;border-radius:3px 3px 0 0"></div><div style="flex:1;background:#FFC93C;height:85%;border-radius:3px 3px 0 0"></div></div></div>
''', "E98 · 双折叠 火花收益"))

# ===== 18-50: 通用功能页（信息卡式模板） =====
def info_page(title, label, items, items_per_row=1, icon_bg="#E8F4FF"):
    rows = "".join(f'<div style="display:flex;gap:9px;align-items:center;background:#fff;border-radius:11px;padding:11px 12px"><span style="width:32px;height:32px;border-radius:9px;background:{icon_bg};display:flex;align-items:center;justify-content:center;font-size:14px;flex-shrink:0">{ic}</span><div style="flex:1"><b style="font-size:12px">{a}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">{b}</p></div><span style="color:#99a;font-size:10px;flex-shrink:0">›</span></div>' for ic,a,b in items)
    return f2_shell(title, f'<div style="display:grid;gap:8px">{rows}</div>', label)

L(info_page("我的邀请", "E99 · 双折叠 邀请", [("🎁","我的邀请名额","1120 个 · 已用 0"),("📨","已发送邀请","3 封"),("⏳","待接受邀请","1 封"),("📜","邀请规则","注册即可获得邀请")], icon_bg="#E8F0FF"))
L(info_page("求种", "E100 · 双折叠 求种", [("🙋","小学数学思维训练 4年级","进行中 · 3人已求"),("🙋","高中生物实验视频 4K","进行中 · 2人已求"),("✅","初中物理竞赛教程","已解决 · 09-01"),("➕","发起新求种","点击填写求种表单")], icon_bg="#E8F8F2"))
L(info_page("字幕", "E101 · 双折叠 字幕", [("🎬","蓝色星球 E01 中英双语","ASS · 09-06"),("🎬","小猪佩奇 S1 英文字幕","SRT · 09-05"),("🎬","纪录片 中华文明 简中","ASS · 09-03"),("➕","上传字幕","发布字幕赚火花")], icon_bg="#FFE8F5"))
L(info_page("站点规则", "E102 · 双折叠 站点规则", [("📜","第一章 总则","站点宗旨与定位"),("👤","第二章 账号","注册、邀请、等级"),("📊","第三章 数据","上传下载分享率"),("📤","第四章 发布","发布规范与奖励"),("⬇️","第五章 下载","下载与保种义务"),("🚫","第六章 违规","违规行为与处罚")], icon_bg="#FFF8E8"))
L(info_page("帮助 FAQ", "E103 · 双折叠 帮助FAQ", [("🏠","站点信息","什么是好学PT？等级说明"),("👤","用户信息","资料修改、密码找回"),("📊","数据统计","分享率如何计算"),("📤","发布","如何发布种子"),("⬇️","下载","下载慢怎么办"),("🌐","网络问题","Tracker连接故障排查")], icon_bg="#E8F4FF"))
L(info_page("工具箱", "E104 · 双折叠 工具箱", [(ic,nm,ds) for ic,nm,ds in TOOLS], icon_bg="#E8F0FF"))
L(info_page("管理组", "E105 · 双折叠 管理组", [(ic, nm, f"{cnt} · {st}") for ic,nm,cnt,st in [("🎧","一线客服","10 人","可申请"),("💬","批评家","5 人","可申请"),("🛡️","论坛版主","8 人","可申请"),("⚙️","常规管理员","6 人","可申请"),("👑","VIP","45 人","可申请")]], icon_bg="#E8F4FF"))
L(info_page("保种区", "E106 · 双折叠 保种区", [("🛡️","官方保种","官方制作组发布≥5天且做种"),("🛡️","全部保种","所有发布≥5天且做种"),("💡","机制说明","做种>7天自动移出，免费延续3天"),("📂","分类","教育/高中部/高职部/初中部/小学部")], icon_bg="#E8F8F2"))
L(info_page("官种", "E107 · 双折叠 官种", [(t[0][:14], f"{t[1]} · {t[2]}", "") for t in OFFICIALS], icon_bg="#FFF8E8"))
L(info_page("娱乐市场", "E108 · 双折叠 娱乐市场", [(ic,nm,ds) for ic,nm,ds,_ in GAMES], icon_bg="#FFE8F5"))
L(info_page("装饰品中心", "E109 · 双折叠 装饰品", [("🖼️","头像框-星光","限定 · 5,000火花"),("🌈","ID彩虹特效","个性 · 200,000火花"),("✨","昵称发光","个性 · 10,000火花"),("🎀","装扮背景","学习风 · 3,000火花")], icon_bg="#FFE8F5"))
L(info_page("我的收藏", "E110 · 双折叠 我的收藏", [("⭐","识典古籍 六書正譌","收藏于 09-06"),("⭐","蓝色星球 第三季","收藏于 09-03"),("⭐","高中英语词汇 3500","收藏于 08-28"),("⭐","人教版语文四年级","收藏于 08-20")], icon_bg="#FFF8E8"))
L(info_page("PT人生", "E111 · 双折叠 PT人生", [("🎯","我的等级","发布员 · 下一级 主管"),("📈","成长轨迹","加入 342 天"),("🏅","成就解锁","12 / 40"),("🎮","游戏记录","五子棋 18胜12负"),("💎","总数据","上传108.97T 下载2.83T")], icon_bg="#E8F4FF"))
L(info_page("公告中心", "E112 · 双折叠 公告中心", [(ic,nm,f"{t} · {ds}") for ic,nm,t,ds in NEWS], icon_bg="#E8F4FF"))
L(info_page("站点事件", "E113 · 双折叠 站点事件", [(f"📋","{t} {nm}",ds) for t,nm,_,ds in EVENTS], icon_bg="#E8F8F2"))
L(info_page("在线用户", "E114 · 双折叠 在线用户", [("🟢","在线用户 2,226 人","24小时内活跃"),("🔵","正在下载","282 人"),("🔴","正在做种","142,102 人"),("🟣","今日访问","2,226")], icon_bg="#E8F0FF"))
L(info_page("站点统计", "E115 · 双折叠 站点统计", [("🧲","种子总数","19,434 · 断种 199"),("📦","种子总大小","85.550 TB"),("🔄","同伴数","142,384"),("👥","注册用户","4,645 / 上限 5,000"),("📤","总上传","4.196 PB"),("📥","总下载","467.770 TB")], icon_bg="#E8F4FF"))
L(info_page("最新上传", "E116 · 双折叠 最新上传", [(t[1], t[0][:20], f"{t[3]} · {t[4]}") for t in TORRENTS[:6]], icon_bg="#E8F8F2"))
L(info_page("热门种子", "E117 · 双折叠 热门种子", [("🔥","小猪佩奇 英文版 1-6季","210 下载 · 56 做种"),("🔥","高中数学必修一 同步精讲","156 下载 · 41 做种"),("🔥","人教版语文四年级上册","128 下载 · 34 做种"),("🔥","幼儿识字 3000 字","120 下载 · 19 做种")], icon_bg="#FFF8E8"))
L(info_page("断种列表", "E118 · 双折叠 断种", [("⚠️","蓝色星球 S2 4K","0 做种 · 142GB"),("⚠️","三年级英语上册 旧版","0 做种 · 1.2GB"),("⚠️","高中数学选修 旧教材","0 做种 · 3.4GB")], icon_bg="#FFE8E8"))
L(info_page("免费种子", "E119 · 双折叠 免费种子", [("🎉","识典古籍 六書正譌","免费 · 剩 2天"),("🎉","小猪佩奇 英文版","免费 · 剩 5天"),("🎉","人教版语文四年级","免费 · 剩 1天")], icon_bg="#E8F8F2"))
L(info_page("标签云", "E120 · 双折叠 标签云", [("🏷️","Free","1,203 个种子"),("🏷️","保种","856 个"),("🏷️","官方","1,234 个"),("🏷️","古籍","189 个"),("🏷️","2x","672 个"),("🏷️","课本","445 个"),("🏷️","双语","320 个")], icon_bg="#E8F4FF"))
L(info_page("绩效考核", "E121 · 双折叠 绩效考核", [("🏆","保种员 5T 版","月领 200,000 火花"),("📊","操作总数","28 / 30（要求30/最低10）"),("✅","通过审核","25 / 30（要求30/最低10）"),("📜","领取历史","gntv · 冷冷的风 · alan5914")], icon_bg="#E8F0FF"))
L(info_page("论坛最新帖", "E122 · 双折叠 论坛最新帖", [("💬","IYUU辅种保姆级教程","gntv · 09-06"),("💬","新人必读：站点规则","管理组 · 09-05"),("💬","求推荐高中化学资料","study_mom · 09-05"),("💬","发邀：新手友好","kiririn · 09-04")], icon_bg="#E8F8F2"))
L(info_page("我的消息设置", "E123 · 双折叠 消息设置", [("🔔","接收站内通知","开启"),("📧","接收邮件通知","开启"),("📲","接收手机推送","关闭"),("🔄","会话提醒频率","即时")], icon_bg="#E8F4FF"))
L(info_page("举报中心", "E124 · 双折叠 举报", [("🚩","举报规则","如实举报，禁止滥用"),("🚩","举报记录","3 条处理中"),("🆕","发起举报","选择举报类型与对象"),("⚖️","处理进度","平均 24h 内处理")], icon_bg="#FFE8E8"))
L(info_page("好友列表", "E125 · 双折叠 好友", [("👥","alan5914","上传 16,348 · 好友"),("👥","study_mom","上传 892 · 好友"),("👥","kiririn","上传 3,204 · 好友"),("➕","添加好友","输入用户名搜索")], icon_bg="#E8F8F2"))
L(info_page("访问日志", "E126 · 双折叠 访问日志", [("🕐","最近访问","识典古籍 六書正譌 · 10:20"),("🕐","最近访问","蓝色星球 第三季 · 09:45"),("🕐","最近访问","高中数学必修一 · 昨天")], icon_bg="#E8F0FF"))

# ===== 用户详情 ×3 =====
user_tabs = ["概览","上传种子","做种种子","下载历史","评论","收藏","好友","勋章"]
for ui in range(3):
    if ui == 0:
        body = f'''<div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px;display:flex;gap:10px;align-items:center"><img src="{OWL}" style="width:46px;height:46px;border-radius:50%"><div><b style="font-size:15px">gntv</b><p style="color:#99a;font-size:10px;margin:2px 0 0">发布员 · 加入 2025-07-16 · 邮箱 95836184@qq.com</p><p style="color:#99a;font-size:10px;margin:2px 0 0">当前活动 14,247 · 连接数无限制</p></div></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px"><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px;color:#2FA8FF">108.97T</b><p style="color:#99a;font-size:9px;margin:1px 0 0">上传</p></div><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px;color:#2FBF9B">2.83T</b><p style="color:#99a;font-size:9px;margin:1px 0 0">下载</p></div><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px">38.5</b><p style="color:#99a;font-size:9px;margin:1px 0 0">分享率</p></div><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px">14,247</b><p style="color:#99a;font-size:9px;margin:1px 0 0">做种中</p></div></div>'''
    else:
        body = f'''<div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:10px"><b style="font-size:13px">{user_tabs[ui]}列表</b></div><div style="display:grid;gap:8px">{"".join(f2_card_row(t[1], t[2], t[0][:22]+"…" if len(t[0])>22 else t[0], f"{t[3]} · {t[4]} · {t[5]}做种", tag(t[6].split("/")[0])) for t in TORRENTS[:4])}</div>'''
    L(f2_shell("用户详情", f'<div style="display:flex;gap:5px;margin-bottom:10px;flex-wrap:wrap">{"".join(chip(x, on=(i==ui)) for i,x in enumerate(user_tabs))}</div>{body}', f"E12{7+ui} · 双折叠 用户详情-{user_tabs[ui]}"))

# ===== 管理后台 ×6 =====
admin_pages = [
    ("仪表盘", "📊", [("🧲","种子总数","19,434"),("👥","注册用户","4,645"),("🔄","同伴数","142,384"),("📊","待办审核","12")], "今日审核 45 · 通过率 71.1% · 维护模式 正常"),
    ("种子管理", "🧲", [("⏳","待审核种子","12"),("🚩","违规举报","3"),("🏅","官种数量","1,234"),("📦","种子总数","19,434")], "最新待审：高中英语词汇 3500 · 08-30"),
    ("用户管理", "👥", [("👥","注册用户","4,645"),("⚠️","未验证","39"),("🚫","被禁用户","1,341"),("👑","贵宾","45")], "新注册：3 人 · 待审核晋升 2 人"),
    ("公告管理", "📢", [("📢","站点公告","18"),("📝","草稿","3"),("⏰","定时发布","1"),("📊","总阅读","45,892")], "最新公告：站免池月度进度通报"),
    ("勋章管理", "🎖️", [("🏅","勋章总数","81"),("🃏","卡牌总数","45"),("📤","已发放","1,208"),("⏳","待审批","5")], "最新上架：开学季限定勋章"),
    ("系统管理", "🔒", [("⚙️","维护模式","正常"),("💾","备份管理","45 份"),("🔑","API 状态","正常"),("📋","安全设置","已开启")], "最近备份：09-07 02:00 · 12.8GB"),
]
for pi, (pn, pic, stats, note) in enumerate(admin_pages):
    L(f2_shell("管理后台", f'''
          <div style="background:linear-gradient(135deg,#1F2A44,#2d3a5c);border-radius:12px;padding:12px;color:#fff;margin-bottom:10px;display:flex;align-items:center;gap:9px"><span style="font-size:20px">{pic}</span><b style="font-size:14px">{pn}</b><span style="flex:1;text-align:right;font-size:10px;opacity:.8">管理员 gntv</span></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:10px">{"".join(f'<div style="background:#fff;border-radius:10px;padding:10px"><p style="color:#99a;font-size:9px;margin:0">{ic} {nm}</p><b style="font-size:15px">{v}</b></div>' for ic,nm,v in stats)}</div>
          <div style="background:#FFF8E8;border-radius:10px;padding:10px;font-size:11px;color:#667">📌 {note}</div>
''', f"E13{0+pi} · 双折叠 后台-{pn}"))

# ===== 发布类 ×4 =====
L(f2_shell("发布种子", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;display:grid;gap:10px"><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">种子文件 *</label><div style="border:2px dashed #2FA8FF;border-radius:10px;padding:14px;text-align:center;color:#2FA8FF;font-size:12px">📎 点击选择 .torrent</div></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">标题 *</label><input class="gi" placeholder="例：识典古籍 六書正譌" style="width:100%;height:36px;font-size:12px"></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">副标题</label><input class="gi" placeholder="教材/章节/栏目/资源" style="width:100%;height:36px;font-size:12px"></div><div style="display:grid;grid-template-columns:1fr 1fr;gap:8px"><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">类型</label><select class="gi" style="width:100%;height:36px;font-size:11px"><option>教育</option><option>影音</option></select></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">媒介</label><select class="gi" style="width:100%;height:36px;font-size:11px"><option>书籍</option><option>视频</option></select></div></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">简介 *</label><textarea class="gi" placeholder="资源介绍…" style="width:100%;height:56px;font-size:11px;resize:none"></textarea></div></div>
          <button class="f2-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px;margin-top:10px">🚀 发布种子</button>
''', "E136 · 双折叠 发布种子"))
L(f2_shell("发布字幕", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;display:grid;gap:10px"><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">字幕文件 *</label><div style="border:2px dashed #2FBF9B;border-radius:10px;padding:14px;text-align:center;color:#2FBF9B;font-size:12px">📎 点击选择 .ass/.srt</div></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">关联种子</label><input class="gi" placeholder="输入种子ID或名称" style="width:100%;height:36px;font-size:12px"></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">语言</label><div style="display:flex;gap:6px">{"".join(chip(x, on=(i==0)) for i,x in enumerate(["简体中文","繁体中文","英语","双语"]))}</div></div></div>
          <button class="f2-btn" style="background:#2FBF9B;color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px;margin-top:10px">📤 发布字幕</button>
''', "E137 · 双折叠 发布字幕"))
L(f2_shell("提交候选", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;display:grid;gap:10px"><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">候选名称 *</label><input class="gi" placeholder="你想看到什么资源？" style="width:100%;height:36px;font-size:12px"></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">分类</label><select class="gi" style="width:100%;height:36px;font-size:11px"><option>学前教育</option><option>小学</option><option>初中</option><option>高中</option></select></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">理由</label><textarea class="gi" placeholder="为什么需要这个资源？" style="width:100%;height:56px;font-size:11px;resize:none"></textarea></div></div>
          <button class="f2-btn" style="background:#FF7A59;color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px;margin-top:10px">🙋 提交候选</button>
          <div style="background:#FFF8E8;border-radius:10px;padding:10px;margin-top:10px;font-size:11px;color:#667">💡 候选票数达标后由管理组审核发布</div>
''', "E138 · 双折叠 提交候选"))
L(f2_shell("发起求种", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;display:grid;gap:10px"><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">求种标题 *</label><input class="gi" placeholder="例：小学数学思维训练 4年级" style="width:100%;height:36px;font-size:12px"></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">描述</label><textarea class="gi" placeholder="资源详情、版本要求…" style="width:100%;height:64px;font-size:11px;resize:none"></textarea></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">悬赏火花</label><input class="gi" value="5,000" style="width:100%;height:36px;font-size:12px"></div></div>
          <button class="f2-btn" style="background:#2FBF9B;color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px;margin-top:10px">🙏 发布求种</button>
''', "E139 · 双折叠 发起求种"))

# ===== 规则/搜索/登录注册/404 =====
L(f2_shell("搜索", f'''
          <div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:10px;display:flex;gap:8px"><input class="gi" placeholder="搜索种子/字幕/帖子…" style="flex:1;height:38px;font-size:13px"><span style="background:#2FA8FF;color:#fff;border-radius:10px;padding:0 16px;display:flex;align-items:center;font-size:13px">搜索</span></div>
          <div style="background:#fff;border-radius:10px;padding:10px 12px;margin-bottom:10px;font-size:12px;color:#667">找到 <b style="color:#2FA8FF">24</b> 个与「<b>古籍</b>」相关的结果</div>
          <div style="display:grid;gap:8px">{"".join(f2_card_row(t[1], t[2], t[0], f"{t[3]} · {t[4]} · {t[5]}做种", tag(t[6].split("/")[0])) for t in TORRENTS[:4])}</div>
''', "E140 · 双折叠 搜索"))
L(f2_shell("站点规则", f'''
          <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:14px">📜 好学PT 站点规则</b><p style="color:#99a;font-size:10px;margin:4px 0 0">更新于 2026-01-01</p><div style="display:grid;gap:10px;margin-top:12px"><div><b style="font-size:13px;color:#2FA8FF">一、账号规范</b><p style="color:#667;font-size:11px;line-height:1.8;margin:4px 0 0">一个IP仅注册一个账号，禁止马甲；账号禁止转借共享出售；120天不活跃可能被删除。</p></div><div><b style="font-size:13px;color:#FF7A59">二、分享率规范</b><p style="color:#667;font-size:11px;line-height:1.8;margin:4px 0 0">分享率低于 0.3 限制下载；禁止任何作弊手段。</p></div><div><b style="font-size:13px;color:#2FBF9B">三、资源规范</b><p style="color:#667;font-size:11px;line-height:1.8;margin:4px 0 0">禁止虚假种子；正确选择分类年级版本。</p></div><div><b style="font-size:13px;color:#5B6BF5">四、违规处理</b><p style="color:#667;font-size:11px;line-height:1.8;margin:4px 0 0">作弊、侵权、恶意行为将视情节警告、降级、封禁。</p></div></div></div>
''', "E141 · 双折叠 站点规则"))


def fc_tor(t):
    return f'<div style="background:#fff;border-radius:10px;padding:10px;display:flex;gap:8px;align-items:center"><span style="width:28px;height:28px;border-radius:8px;background:{t[2]};color:#fff;display:flex;align-items:center;justify-content:center;font-size:11px;flex-shrink:0">{t[1]}</span><div style="flex:1;min-width:0"><b style="font-size:11px;display:block;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{t[0][:18]}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">{t[4]} · {t[5]}做种</p></div></div>'



# ===== 外屏 15页 =====
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
L(fc_shell("首页", f'''
          <div style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);border-radius:14px;padding:14px;color:#fff;margin-bottom:10px"><b style="font-size:15px">好学者如春苗 🌱</b><p style="font-size:10px;opacity:.9;margin:4px 0 0">19,434 个学习资源</p><span style="background:rgba(255,255,255,.2);border-radius:9px;padding:5px 12px;font-size:10px;display:inline-block;margin-top:8px">浏览资源</span></div>
          <div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:7px;margin-bottom:10px"><div style="background:#fff;border-radius:10px;padding:9px;text-align:center"><b style="font-size:13px">108.97T</b><p style="color:#99a;font-size:9px;margin:1px 0 0">上传</p></div><div style="background:#fff;border-radius:10px;padding:9px;text-align:center"><b style="font-size:13px">38.5</b><p style="color:#99a;font-size:9px;margin:1px 0 0">分享率</p></div><div style="background:#fff;border-radius:10px;padding:9px;text-align:center"><b style="font-size:13px">85.8M</b><p style="color:#99a;font-size:9px;margin:1px 0 0">火花</p></div></div>
          <div style="font-size:12px;font-weight:700;margin-bottom:7px">最新资源</div>
          <div style="display:grid;gap:7px">{"".join(fc_tor(t) for t in TORRENTS[:3])}</div>
''', "E142 · 双折叠 外屏-首页"))
L(fc_shell("浏览", f'''
          <div style="display:flex;gap:5px;margin-bottom:8px;flex-wrap:wrap">{"".join(chip(c, on=(i==0)) for i,(c,_) in enumerate(cats[:4]))}</div>
          <div style="display:grid;gap:7px">{"".join(fc_tor(t) for t in TORRENTS[:5])}</div>
''', "E143 · 双折叠 外屏-浏览"))
L(fc_shell("消息", f'''
          <div style="background:#E8F4FF;border-radius:10px;padding:10px;margin-bottom:7px"><b style="font-size:12px">🌟 管理组</b><p style="color:#667;font-size:11px;margin:4px 0 0">您的种子已设为官种</p></div>
          <div style="background:#fff;border-radius:10px;padding:10px;margin-bottom:7px"><b style="font-size:12px">📢 站免池</b><p style="color:#667;font-size:11px;margin:4px 0 0">进度 52.3%</p></div>
          <div style="background:#fff;border-radius:10px;padding:10px;margin-bottom:7px"><b style="font-size:12px">💬 alan5914</b><p style="color:#667;font-size:11px;margin:4px 0 0">求种回复：已发布 ✓</p></div>
''', "E144 · 双折叠 外屏-消息", "💬"))
L(fc_shell("站免池", f'''
          <div style="background:linear-gradient(135deg,#FF7A59,#FFC93C);border-radius:14px;padding:14px;color:#fff;margin-bottom:9px"><p style="font-size:10px;opacity:.9;margin:0">站免池</p><b style="font-size:19px">52.3%</b><div style="height:7px;background:rgba(255,255,255,.3);border-radius:4px;margin-top:5px;overflow:hidden"><div style="width:52.3%;height:100%;background:#fff;border-radius:4px"></div></div></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:7px">{"".join(f'<div style="background:#fff;border-radius:10px;padding:10px;text-align:center"><b style="font-size:12px;color:#FF7A59">{t}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">火花</p></div>' for t in POOL_TIERS[:4])}</div>
          <button style="width:100%;background:#FF7A59;color:#fff;border:none;border-radius:10px;padding:11px;font-size:13px;margin-top:9px">💛 快速捐赠</button>
''', "E145 · 双折叠 外屏-站免池"))
L(fc_shell("银行", f'''
          <div style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);border-radius:14px;padding:14px;color:#fff;margin-bottom:9px"><p style="font-size:10px;opacity:.9;margin:0">总资产</p><b style="font-size:18px">85,814,940</b><p style="font-size:9px;opacity:.9;margin:3px 0 0">+469/h</p></div>
          <div style="background:#fff;border-radius:10px;padding:10px;font-size:12px;margin-bottom:7px;display:flex;justify-content:space-between"><span>活期余额</span><b>3,321.81</b></div>
          <div style="background:#fff;border-radius:10px;padding:10px;font-size:12px;margin-bottom:7px;display:flex;justify-content:space-between"><span>在投定期</span><b>0</b></div>
          <div style="background:#fff;border-radius:10px;padding:10px;font-size:12px;display:flex;justify-content:space-between"><span>贷款负债</span><b>0</b></div>
''', "E146 · 双折叠 外屏-银行"))
L(fc_shell("农场", f'''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:7px">{"".join(f'<div style="background:#fff;border-radius:10px;padding:10px;text-align:center"><span style="font-size:20px">{ic}</span><b style="display:block;font-size:11px;margin-top:2px">{nm}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">{"生长中" if i==0 else "空闲"}</p></div>' for i,(ic,nm,tm,v) in enumerate(CROPS))}</div>
          <div style="background:#FFF8E8;border-radius:9px;padding:9px;margin-top:9px;font-size:10px;color:#667">🌾 小麦即将成熟 · 2h</div>
''', "E147 · 双折叠 外屏-农场"))
L(fc_shell("签到", f'''
          <div style="background:linear-gradient(135deg,#2FBF9B,#2FA8FF);border-radius:14px;padding:14px;color:#fff;text-align:center;margin-bottom:9px"><b style="font-size:22px">120 天</b><p style="font-size:10px;opacity:.9;margin:2px 0 0">已连续签到</p><div style="background:rgba(255,255,255,.2);border-radius:12px;padding:6px 14px;display:inline-block;font-size:11px;margin-top:8px">✅ 今日已签</div></div>
          <div style="background:#fff;border-radius:10px;padding:10px;font-size:11px;color:#667">🎁 连续奖励：30天1000 / 100天10000+勋章</div>
''', "E148 · 双折叠 外屏-签到"))
L(fc_shell("任务", f'''
          <div style="display:grid;gap:7px">{"".join(f'<div style="display:flex;align-items:center;gap:8px;background:#fff;border-radius:10px;padding:10px;font-size:11px"><span style="font-size:16px">{ic}</span><b style="flex:1">{nm}</b><span style="background:{c}20;color:{c};border-radius:9px;padding:2px 7px;font-size:9px">{st}</span></div>' for ic,nm,rw,st,c in TASKS[:5])}</div>
''', "E149 · 双折叠 外屏-任务"))
L(fc_shell("勋章", f'''
          <div style="background:#fff;border-radius:10px;padding:10px;margin-bottom:8px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:12px">我的收藏 30/81</b><span style="color:#FFC93C">★★★★★</span></div>
          <div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:7px">{"".join(f'<div style="background:#fff;border-radius:9px;padding:8px;text-align:center;{"opacity:.4" if i>=4 else ""}"><span style="font-size:18px">{m}</span><p style="font-size:8px;margin:2px 0 0;color:#667">{nm}</p></div>' for i,(m,nm,ds) in enumerate(MEDALS[:9]))}</div>
''', "E150 · 双折叠 外屏-勋章"))
L(fc_shell("排行", f'''
          <div style="display:grid;gap:7px">{"".join(f'<div style="display:flex;align-items:center;gap:8px;background:#fff;border-radius:10px;padding:9px 10px"><span style="width:18px;text-align:center;font-weight:700;color:{"#FFC93C" if i<3 else "#99a"}">{i+1}</span><img src="{OWL}" style="width:26px;height:26px;border-radius:50%"><b style="flex:1;font-size:11px">{u}</b><b style="font-size:10px;color:#FF7A59">{v}</b></div>' for i,(u,cl,v,_) in enumerate(RANKS))}</div>
''', "E151 · 双折叠 外屏-排行"))
L(fc_shell("商店", f'''
          <div style="display:grid;gap:7px">{"".join(f'<div style="display:flex;justify-content:space-between;align-items:center;background:#fff;border-radius:10px;padding:10px;font-size:11px"><b>{nm}</b><b style="color:#FF7A59">{pr}</b></div>' for nm,pr,_ in SHOP[:6])}</div>
''', "E152 · 双折叠 外屏-商店"))
L(fc_shell("课本", f'''
          <div style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);border-radius:12px;padding:12px;color:#fff;margin-bottom:8px"><b style="font-size:13px">📚 课本中心</b><p style="font-size:9px;opacity:.9;margin:2px 0 0">一键下载小学初中高中课本</p></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:7px">{"".join(f'<div style="background:#fff;border-radius:10px;padding:10px;text-align:center"><span style="font-size:18px">📘</span><b style="display:block;font-size:11px;margin-top:2px">{s}</b></div>' for s in SUBJECTS[:8])}</div>
''', "E153 · 双折叠 外屏-课本"))
L(fc_shell("论坛", f'''
          <div style="display:grid;gap:7px">{"".join(f'<div style="background:#fff;border-radius:10px;padding:10px"><b style="font-size:12px;color:{c}">{fn}</b><p style="color:#99a;font-size:9px;margin:3px 0 0">{" · ".join(f"{s}({cnt})" for s,cnt in subs[:2])}</p></div>' for fn,subs,c in FORUMS)}</div>
''', "E154 · 双折叠 外屏-论坛", "💬"))
L(fc_shell("我的", f'''
          <div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:8px;display:flex;align-items:center;gap:9px"><img src="{OWL}" style="width:38px;height:38px;border-radius:50%"><div><b style="font-size:13px">gntv</b><p style="color:#99a;font-size:9px;margin:1px 0 0">发布员</p></div></div>
          <div style="display:grid;gap:7px">{"".join(f2_card_row(ic, "#F0F7FF", nm, "", f'<span style="color:#99a;font-size:10px">{v}</span>') for ic,nm,v in MYMENU[:6])}</div>
''', "E155 · 双折叠 外屏-我的", "👤"))

html = "\n".join(out)
open('/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/gen/fold2_boards.html','w',encoding='utf-8').write(html)
print("fold2 boards:", len(out), "last id:", n[0])

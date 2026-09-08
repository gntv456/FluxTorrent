# -*- coding: utf-8 -*-
"""形态全覆盖: 为每个全展开页面生成 外屏(f2c)/半展开(f3h)/折叠态(f3c) 版本, 全部对齐"""
import re, sys
sys.path.insert(0, '/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/gen')
from data import *

SRC = '/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/index.html'
s = open(SRC, encoding='utf-8').read()
seg = re.search(r'<dc-section id="fold".*?</dc-section>', s, re.S).group(0)

def grab(seg, suf):
    return re.findall(r'<dc-artboard id="(e\d+-'+suf+r')" label="([^"]+)" width="\d+" height="\d+">(.*?)</dc-artboard>', seg, re.S)

f2p = grab(seg, 'f2p')
f3p = grab(seg, 'f3p')
print("f2p:", len(f2p), "f3p:", len(f3p))

def clean(t):
    return re.sub(r'<[^>]+>', '', t).strip()

def theme_label(label):
    parts = label.split('·')
    return parts[-1].strip() if len(parts) >= 2 else label

def extract_items(html, maxn=6):
    bs = re.findall(r'<b[^>]*>(.*?)</b>', html, re.S)
    items = []
    for b in bs:
        t = clean(b)
        if t and t not in items and len(t) <= 24:
            items.append(t)
        if len(items) >= maxn:
            break
    return items

def theme_meta(t):
    pairs = [
        ("首页","🏠","#2FA8FF"),("资源","📚","#5B6BF5"),("浏览","🔍","#2FA8FF"),("种子","📦","#2FBF9B"),("详情","📄","#5B6BF5"),
        ("文件","🗂️","#FF7A59"),("做种","🌱","#2FBF9B"),("下载","⬇️","#2FA8FF"),("评论","💬","#FF8FC7"),("发布","📤","#FF7A59"),
        ("论坛","💬","#5B6BF5"),("帖子","📜","#2FA8FF"),("课本","📖","#2FBF9B"),("勋章","🎖️","#FFC93C"),("卡牌","🃏","#8B5CF6"),
        ("排行","🏆","#FF7A59"),("商店","🛒","#FFC93C"),("银行","🏦","#5B6BF5"),("站免池","💛","#FF7A59"),("农场","🌾","#2FBF9B"),
        ("菜市场","🥬","#2FBF9B"),("游戏","🎮","#8B5CF6"),("五子棋","⚫","#1F2A44"),("刮刮乐","🎫","#FF7A59"),("九宫格","🎯","#5B6BF5"),
        ("猜大小","🎲","#2FBF9B"),("合成","✨","#8B5CF6"),("工具","🛠️","#8895aa"),("图床","🖼️","#2FA8FF"),("IYUU","🔄","#2FBF9B"),
        ("我的数据","👤","#2FA8FF"),("绩校","📊","#5B6BF5"),("装饰","🎀","#FF8FC7"),("待审核","⏳","#FF7A59"),("禁止","🚫","#FF5A5A"),
        ("管理组","🛡️","#5B6BF5"),("PM","📮","#2FBF9B"),("日志","📋","#8895aa"),("守护神","🛡️","#8B5CF6"),("发送","📨","#2FA8FF"),
        ("退出","🚪","#FF5A5A"),("消息","💌","#FF8FC7"),("收件箱","📥","#2FA8FF"),("会话","💬","#5B6BF5"),("发件箱","📤","#2FBF9B"),
        ("回收站","🗑️","#FF5A5A"),("草稿","📝","#FFC93C"),("设置","⚙️","#8895aa"),("提交","📋","#2FA8FF"),("创建","➕","#2FBF9B"),
        ("后台","🛠️","#8895aa"),("公告","📢","#FF7A59"),("事件","📋","#2FA8FF"),("帮助","❓","#2FBF9B"),("规则","📜","#8895aa"),
        ("常见问题","❓","#2FA8FF"),("关于","ℹ️","#8895aa"),("捐赠","💛","#FF7A59"),("友链","🔗","#2FA8FF"),("联系","📮","#5B6BF5"),
        ("请求","🙋","#FF8FC7"),("求种","🙏","#FF8FC7"),("字幕","💬","#2FA8FF"),("保种","🛡️","#2FBF9B"),("官种","🏅","#FFC93C"),
        ("分类","🗂️","#2FA8FF"),("搜索","🔍","#2FA8FF"),("筛选","🎚️","#8895aa"),("排序","↕️","#8895aa"),("分页","📄","#8895aa"),
        ("网格","🔲","#5B6BF5"),("兑换","🎁","#FF7A59"),("邀请","🎁","#FF8FC7"),("签到","✅","#2FBF9B"),("奖励","🎁","#FFC93C"),
        ("任务","📋","#2FA8FF"),("活动","🎉","#FF7A59"),("收藏","⭐","#FFC93C"),("好友","👥","#2FA8FF"),("收益","💰","#FFC93C"),
        ("火花","🔥","#FF7A59"),("余额","💰","#FFC93C"),("档案","📁","#8895aa"),("求种","🙏","#FF8FC7"),
    ]
    for k, ic, c in pairs:
        if k in t:
            return ic, c
    return "📄", "#5B6BF5"

n = [991]
def next_id():
    n[0] += 1
    return f"e{n[0]}"

# ===== 1. 外屏 f2c (340x720): 单栏紧凑 =====
f2c_out = []
for aid, label, html in f2p:
    t = theme_label(label)
    items = extract_items(html, 5)
    if not items:
        items = [t, "查看详情", "更多"]
    ic, col = theme_meta(t)
    nid = next_id()
    rows = "".join(f'<div style="background:#fff;border-radius:9px;padding:9px 10px;font-size:10px;margin-bottom:6px;display:flex;gap:7px;align-items:center"><span style="width:24px;height:24px;border-radius:7px;background:{col};color:#fff;display:flex;align-items:center;justify-content:center;font-size:10px;flex-shrink:0">{ic}</span><b style="flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{it}</b></div>' for it in items)
    f2c_out.append(f'''    <dc-artboard id="{nid}-f2c" label="E{nid[1:]} · 双折叠 外屏-{t}" width="340" height="720">
      <div class="fold2-closed">
        <div class="fc-statusbar"><span>9:41</span><span>📶 🔋</span></div>
        <div class="fc-topbar"><div class="fc-logo"><img src="{OWL}" alt=""><b>{t.split("-")[0][:8]}</b></div><span>🔔</span></div>
        <div class="fc-scroll">
          <div style="background:linear-gradient(135deg,{col},#5B6BF5);border-radius:12px;padding:13px;color:#fff;margin-bottom:8px"><p style="font-size:10px;opacity:.9;margin:0">{t.split("-")[0][:9]}</p><b style="font-size:17px;display:block;margin-top:3px">{ic} {t[:10]}</b></div>
          {rows}
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:7px"><div style="background:linear-gradient(135deg,{col},#5B6BF5);border-radius:10px;padding:10px;color:#fff;text-align:center;font-size:11px">🔍 查看详情</div><div style="background:#fff;border-radius:10px;padding:10px;text-align:center;font-size:11px;color:{col};border:1px solid {col}">💬 快捷操作</div></div>
        </div>
        <div class="fc-dock"><span>🏠</span><span style="background:{col};color:#fff;border-radius:10px">{ic}</span><span>📥</span><span>👤</span></div>
      </div>
    </dc-artboard>
''')

# ===== 2. 半展开 f3h (900x620): 左内容+右数据 =====
f3h_out = []
for aid, label, html in f3p:
    t = theme_label(label)
    items = extract_items(html, 6)
    if not items:
        items = [t, "查看详情", "更多"]
    ic, col = theme_meta(t)
    nid = next_id()
    left = "".join(f'<div style="background:#fff;border-radius:10px;padding:9px 12px;font-size:11px;margin-bottom:7px;display:flex;gap:8px;align-items:center"><span style="width:28px;height:28px;border-radius:8px;background:{col};color:#fff;display:flex;align-items:center;justify-content:center;font-size:11px;flex-shrink:0">{ic}</span><b style="flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{it}</b></div>' for it in items)
    right = f'''
        <div style="background:linear-gradient(135deg,{col},#5B6BF5);border-radius:11px;padding:12px;color:#fff;margin-bottom:8px"><p style="font-size:10px;opacity:.9;margin:0">页面概览</p><b style="font-size:15px;display:block;margin-top:3px">{ic} {t[:12]}</b></div>
        <div style="background:#fff;border-radius:10px;padding:10px 12px;margin-bottom:7px;font-size:11px"><b>📌 条目数</b><p style="color:#99a;font-size:10px;margin:3px 0 0">{len(items)} 项 · 实时同步</p></div>
        <div style="background:#fff;border-radius:10px;padding:10px 12px;margin-bottom:7px;font-size:11px"><b>⏱️ 更新时间</b><p style="color:#99a;font-size:10px;margin:3px 0 0">2026-09-07 实时</p></div>
        <div style="display:grid;gap:6px"><div style="background:linear-gradient(135deg,{col},#5B6BF5);color:#fff;border-radius:9px;padding:9px;text-align:center;font-size:12px">🔍 查看详情</div><div style="background:#fff;color:{col};border:1px solid {col};border-radius:9px;padding:9px;text-align:center;font-size:12px">💬 快捷操作</div></div>
    '''
    f3h_out.append(f'''    <dc-artboard id="{nid}-f3h" label="E{nid[1:]} · 三折叠 半开-{t}" width="900" height="620">
      <div class="fold3-screen" style="flex-direction:row">
        <div style="flex:1.15;background:#F5FAFF;display:flex;flex-direction:column;overflow:hidden"><div style="background:#fff;padding:10px 13px;font-size:13px;border-bottom:1px solid #EEF2F7;display:flex;gap:8px;align-items:center"><span style="font-size:16px">{ic}</span><b>{t.split("-")[0][:10]}</b></div><div style="padding:11px;flex:1;overflow:hidden">{left}</div></div>
        <div style="width:12px;background:linear-gradient(180deg,{col},#5B6BF5);border-radius:6px;margin:5px 3px"></div>
        <div style="flex:0.85;background:#F5FAFF;display:flex;flex-direction:column;overflow:hidden"><div style="background:#fff;padding:10px 13px;font-size:13px;border-bottom:1px solid #EEF2F7"><b>数据面板</b></div><div style="padding:11px;flex:1;overflow:hidden">{right}</div></div>
      </div>
    </dc-artboard>
''')

# ===== 3. 折叠态 f3c (600x400): 紧凑单栏 =====
f3c_out = []
for aid, label, html in f3p:
    t = theme_label(label)
    items = extract_items(html, 4)
    if not items:
        items = [t, "查看详情"]
    ic, col = theme_meta(t)
    nid = next_id()
    rows = "".join(f'<div style="background:#fff;border-radius:9px;padding:8px 10px;font-size:10px;margin-bottom:6px;display:flex;gap:7px;align-items:center"><span style="width:22px;height:22px;border-radius:6px;background:{col};color:#fff;display:flex;align-items:center;justify-content:center;font-size:9px;flex-shrink:0">{ic}</span><b style="flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{it}</b></div>' for it in items)
    f3c_out.append(f'''    <dc-artboard id="{nid}-f3c" label="E{nid[1:]} · 三折叠 折叠态-{t}" width="600" height="400">
      <div class="fold3-closed">
        <div class="fc3-statusbar"><span>9:41</span><span>📶 🔋</span></div>
        <div class="fc3-topbar"><div class="fc3-logo"><img src="{OWL}" alt=""><b>{t.split("-")[0][:8]}</b></div><span>🔔</span></div>
        <div style="padding:12px;flex:1;overflow:hidden">
          <div style="background:linear-gradient(135deg,{col},#5B6BF5);border-radius:11px;padding:11px;color:#fff;margin-bottom:8px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:13px">{ic} {t[:12]}</b><span style="background:rgba(255,255,255,.25);border-radius:8px;padding:3px 10px;font-size:10px">查看</span></div>
          {rows}
        </div>
        <div class="fc3-dock"><span>🏠</span><span style="background:{col};color:#fff;border-radius:8px">{ic}</span><span>📥</span><span>👤</span></div>
      </div>
    </dc-artboard>
''')

html_out = "\n".join(f2c_out + f3h_out + f3c_out)
open('/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/gen/fold_align_boards.html','w',encoding='utf-8').write(html_out)
print("f2c:", len(f2c_out), "f3h:", len(f3h_out), "f3c:", len(f3c_out), "total:", len(f2c_out)+len(f3h_out)+len(f3c_out), "last id:", n[0])

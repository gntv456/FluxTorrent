# -*- coding: utf-8 -*-
"""分屏模式全覆盖: 为每个全展开页面(f2p/f3p)生成对应的双屏分屏(f2s)与三屏分屏(f3s), 数量完全对齐"""
import re, sys
sys.path.insert(0, '/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/gen')
from data import *

SRC = '/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/index.html'
s = open(SRC, encoding='utf-8').read()
seg = re.search(r'<dc-section id="fold".*?</dc-section>', s, re.S).group(0)

# 提取所有 f2p / f3p 画板块
def grab(seg, suf):
    blocks = re.findall(r'<dc-artboard id="(e\d+-'+suf+r')" label="([^"]+)" width="\d+" height="\d+">(.*?)</dc-artboard>', seg, re.S)
    return blocks

f2p = grab(seg, 'f2p')
f3p = grab(seg, 'f3p')
print("f2p:", len(f2p), "f3p:", len(f3p))

def clean(t):
    t = re.sub(r'<[^>]+>', '', t)
    return t.strip()

def theme_label(label):
    # label 形如 "E123 · 双折叠 我的数据-上传" -> 取 "双折叠" 后
    parts = label.split('·')
    if len(parts) >= 2:
        t = parts[-1].strip()
        return t
    return label

def extract_items(html, maxn=8):
    """提取画板内 <b> 文本作为列表项(去重保序)"""
    bs = re.findall(r'<b[^>]*>(.*?)</b>', html, re.S)
    items = []
    for b in bs:
        t = clean(b)
        if t and t not in items and len(t) <= 28:
            items.append(t)
        if len(items) >= maxn:
            break
    return items

n = [568]
def next_id():
    n[0] += 1
    return f"e{n[0]}"

# 主题 -> 图标/颜色映射
def theme_meta(t):
    pairs = [
        ("首页","🏠","#2FA8FF"),("资源","📚","#5B6BF5"),("浏览","🔍","#2FA8FF"),("种子","📦","#2FBF9B"),("详情","📄","#5B6BF5"),
        ("文件","🗂️","#FF7A59"),("做种","🌱","#2FBF9B"),("下载","⬇️","#2FA8FF"),("评论","💬","#FF8FC7"),("发布","📤","#FF7A59"),
        ("论坛","💬","#5B6BF5"),("帖子","📜","#2FA8FF"),("课本","📖","#2FBF9B"),("勋章","🎖️","#FFC93C"),("卡牌","🃏","#8B5CF6"),
        ("排行","🏆","#FF7A59"),("商店","🛒","#FFC93C"),("银行","🏦","#5B6BF5"),("站免池","💛","#FF7A59"),("农场","🌾","#2FBF9B"),
        ("菜市场","🥬","#2FBF9B"),("游戏","🎮","#8B5CF6"),("五子棋","⚫","#1F2A44"),("刮刮乐","🎫","#FF7A59"),("九宫格","🎯","#5B6BF5"),
        ("猜大小","🎲","#2FBF9B"),("卡牌合成","✨","#8B5CF6"),("工具","🛠️","#8895aa"),("图床","🖼️","#2FA8FF"),("IYUU","🔄","#2FBF9B"),
        ("我的数据","👤","#2FA8FF"),("绩校","📊","#5B6BF5"),("装饰","🎀","#FF8FC7"),("待审核","⏳","#FF7A59"),("禁止","🚫","#FF5A5A"),
        ("管理组","🛡️","#5B6BF5"),("PM","📮","#2FBF9B"),("排行","🏅","#FFC93C"),("日志","📋","#8895aa"),("守护神","🛡️","#8B5CF6"),
        ("发送","📨","#2FA8FF"),("退出","🚪","#FF5A5A"),("消息","💌","#FF8FC7"),("收件箱","📥","#2FA8FF"),("会话","💬","#5B6BF5"),
        ("发件箱","📤","#2FBF9B"),("回收站","🗑️","#FF5A5A"),("草稿","📝","#FFC93C"),("设置","⚙️","#8895aa"),
        ("课本-","📗","#2FBF9B"),("我的提交","📋","#2FA8FF"),("管理","⚙️","#8895aa"),("创建","➕","#2FBF9B"),
        ("后台","🛠️","#8895aa"),("公告","📢","#FF7A59"),("事件","📋","#2FA8FF"),("帮助","❓","#2FBF9B"),("规则","📜","#8895aa"),
        ("常见问题","❓","#2FA8FF"),("关于","ℹ️","#8895aa"),("捐赠","💛","#FF7A59"),("友链","🔗","#2FA8FF"),("联系","📮","#5B6BF5"),
        ("请求","🙋","#FF8FC7"),("求种","🙏","#FF8FC7"),("字幕","💬","#2FA8FF"),("保种","🛡️","#2FBF9B"),("官种","🏅","#FFC93C"),
        ("分类","🗂️","#2FA8FF"),("搜索","🔍","#2FA8FF"),("筛选","🎚️","#8895aa"),("排序","↕️","#8895aa"),("分页","📄","#8895aa"),
        ("网格","🔲","#5B6BF5"),("表","📊","#2FA8FF"),("筛选","🎚️","#8895aa"),("兑换","🎁","#FF7A59"),("邀请","🎁","#FF8FC7"),
        ("签到","✅","#2FBF9B"),("奖励","🎁","#FFC93C"),("任务","📋","#2FA8FF"),("活动","🎉","#FF7A59"),("收藏","⭐","#FFC93C"),
        ("好友","👥","#2FA8FF"),("收益","💰","#FFC93C"),("火花","🔥","#FF7A59"),("余额","💰","#FFC93C"),("档案","📁","#8895aa"),
    ]
    for k, ic, c in pairs:
        if k in t:
            return ic, c
    return "📄", "#5B6BF5"

def stat_line(html):
    """提取一个统计值"""
    m = re.search(r'font-size:1[4-6]px[^>]*>([^<>]{2,16})<', html)
    if m:
        v = clean(m.group(1))
        if v and len(v) <= 16:
            return v
    m2 = re.search(r'font-size:2[0-9]px[^>]*>([^<>]{2,14})<', html)
    if m2:
        v = clean(m2.group(1))
        if v and len(v) <= 14:
            return v
    return None

# ===== 双折叠分屏 f2s (左栏=页面内容, 右栏=操作/信息) =====
f2s_out = []
for aid, label, html in f2p:
    t = theme_label(label)
    items = extract_items(html, 9)
    if not items:
        items = [t, "内容加载中", "查看详情", "更多操作"]
    ic, col = theme_meta(t)
    st = stat_line(html)
    nid = next_id()
    left = "".join(f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;margin-bottom:7px">{"".join(ic)} <b>{it}</b></div>' if False else f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;margin-bottom:7px;display:flex;gap:8px;align-items:center"><span style="width:26px;height:26px;border-radius:8px;background:{col};color:#fff;display:flex;align-items:center;justify-content:center;font-size:11px;flex-shrink:0">{ic}</span><b style="flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{it}</b></div>' for it in items[:6])
    right = f'''
        <div style="background:linear-gradient(135deg,{col},#5B6BF5);border-radius:11px;padding:12px;color:#fff;margin-bottom:8px"><p style="font-size:10px;opacity:.9;margin:0">当前页面</p><b style="font-size:16px;display:block;margin-top:3px;word-break:break-all">{t[:12]}</b></div>
        <div style="display:grid;gap:7px;margin-bottom:8px"><div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px"><b>📌 关联条目</b><p style="color:#99a;font-size:10px;margin:3px 0 0">{len(items)} 项</p></div><div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px"><b>{f"📊 关键数值" if st else "⏱️ 更新时间"}</b><p style="color:#99a;font-size:10px;margin:3px 0 0">{st if st else "2026-09-07 实时"}</p></div></div>
        <div style="display:grid;gap:6px"><div style="background:linear-gradient(135deg,{col},#5B6BF5);color:#fff;border-radius:9px;padding:9px;text-align:center;font-size:12px">🔍 查看详情</div><div style="background:#fff;color:{col};border:1px solid {col};border-radius:9px;padding:9px;text-align:center;font-size:12px">💬 快捷操作</div></div>
    '''
    f2s_out.append(f'''    <dc-artboard id="{nid}-f2s" label="E{nid[1:]} · 双折叠 分屏-{t}" width="720" height="900">
      <div class="fold2-screen" style="flex-direction:row">
        <div style="flex:1.15;background:#F5FAFF;display:flex;flex-direction:column;overflow:hidden"><div style="background:#fff;padding:10px 12px;font-size:13px;border-bottom:1px solid #EEF2F7;display:flex;gap:8px;align-items:center"><span style="font-size:16px">{ic}</span><b>{t.split("-")[0][:10]}</b></div><div style="padding:10px;flex:1;overflow:hidden">{left}</div></div>
        <div style="width:10px;background:linear-gradient(180deg,{col},#5B6BF5);border-radius:5px;margin:4px 2px"></div>
        <div style="flex:0.85;background:#F5FAFF;display:flex;flex-direction:column;overflow:hidden"><div style="background:#fff;padding:10px 12px;font-size:13px;border-bottom:1px solid #EEF2F7"><b>快捷面板</b></div><div style="padding:10px;flex:1;overflow:hidden">{right}</div></div>
      </div>
    </dc-artboard>
''')

# ===== 三折叠分屏 f3s (三栏) =====
f3s_out = []
for aid, label, html in f3p:
    t = theme_label(label)
    items = extract_items(html, 12)
    if not items:
        items = [t, "内容加载中", "查看详情", "更多操作", "返回上级"]
    ic, col = theme_meta(t)
    st = stat_line(html)
    nid = next_id()
    def pane(title, icn, colr, its, extra=None):
        rows = "".join(f'<div style="background:#fff;border-radius:9px;padding:8px 10px;font-size:10px;margin-bottom:6px;display:flex;gap:7px;align-items:center"><span style="width:24px;height:24px;border-radius:7px;background:{colr};color:#fff;display:flex;align-items:center;justify-content:center;font-size:10px;flex-shrink:0">{icn}</span><b style="flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{it}</b></div>' for it in its)
        return f'<div style="flex:1;background:#F5FAFF;display:flex;flex-direction:column;overflow:hidden"><div style="background:#fff;padding:9px 11px;font-size:12px;border-bottom:1px solid #EEF2F7;display:flex;gap:7px;align-items:center"><span>{icn}</span><b>{title}</b></div><div style="padding:9px;flex:1;overflow:hidden">{rows}{extra or ""}</div></div>'
    p1 = pane(t.split("-")[0][:9] or t[:9], ic, col, items[:4], f'<div style="background:linear-gradient(135deg,{col},#5B6BF5);border-radius:8px;padding:9px;color:#fff;text-align:center;font-size:11px;margin-top:6px">🔍 查看详情</div>')
    p2 = pane("数据一览", "📊", "#2FA8FF", [f"{t} 共 {len(items)} 项", st or "2026-09-07 更新", "实时同步", "支持快捷操作"], f'<div style="background:#fff;border-radius:8px;padding:8px;font-size:10px;color:#99a;text-align:center;margin-top:6px">⬇️ 下拉刷新</div>')
    p3 = pane("快捷操作", "⚡", "#FF7A59", ["💬 进入评论区", "📤 分享页面", "⭐ 收藏本页", "🔔 消息提醒"], f'<div style="background:#fff;color:{col};border:1px solid {col};border-radius:8px;padding:8px;text-align:center;font-size:11px;margin-top:6px">更多 →</div>')
    f3s_out.append(f'''    <dc-artboard id="{nid}-f3s" label="E{nid[1:]} · 三折叠 分屏-{t}" width="1200" height="520">
      <div class="fold3-screen" style="flex-direction:row">
        {p1}<div style="width:8px;background:linear-gradient(180deg,{col},#5B6BF5);border-radius:4px;margin:4px 2px"></div>{p2}<div style="width:8px;background:linear-gradient(180deg,#FF7A59,#FFC93C);border-radius:4px;margin:4px 2px"></div>{p3}
      </div>
    </dc-artboard>
''')

html_out = "\n".join(f2s_out + f3s_out)
open('/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/gen/fold_split_boards.html','w',encoding='utf-8').write(html_out)
print("f2s generated:", len(f2s_out), "f3s generated:", len(f3s_out), "total:", len(f2s_out)+len(f3s_out), "last id:", n[0])

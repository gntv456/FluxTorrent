# -*- coding: utf-8 -*-
"""常驻闸门 KL：字典强转口点到的键必须都在字典里（P-556 同族的「静默空白」）。

`dict.admin as unknown as Record<string, string>` 这类强转关掉了 tsc 的键检查
（字典本身是 `Dict`，缺键编译不过；转成 Record 就随便点）。后果比 P-556 的
「显示英文字段名」更静：`a.someKey` 拼错时 JSX 渲染成**空白**，职员既看不到
中文也看不到键名，只会以为「这个按钮本来就没说明」。

尺子来自 scripts/audit_dict_cast_keys.py（普查与闸门共用一把）。
今天净结论：144 处访问点 0 缺失——本批没有洞，KL 是防未来漏登记的护栏。
KL2 那两条是仪器自己的账：`cheatersNote` 的值写在下一行、`navSubs` 是嵌套对象，
行级取键的旧写法把这两处都算错过（一假红一假绿），所以钉死。
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, summary  # noqa: E402
import audit_dict_cast_keys as adc  # noqa: E402


def main():
    accesses, keysets, scanned, warns = adc.collect()
    miss = [a for a in accesses if a[1] not in keysets[a[0]][0]]

    ok("KL0 前提：抓到 %d 处强转访问点（>=100）、%d 个直转文件、%d 个子树"
       % (len(accesses), scanned, len(keysets)),
       len(accesses) >= 100 and scanned >= 8 and len(keysets) >= 3,
       "基数掉了先怀疑尺子被改坏，后面的绿不算数")
    ok("KL0b 没有「子树键集取不到」的告警（%d 条）" % len(warns), not warns,
       str(warns)[:200])

    ok("KL1 强转口点到的键都在字典里（缺 %d 处）" % len(miss),
       not miss, "; ".join("%s.%s@%s:%s" % m[:4] for m in miss[:6])[:200])

    leaves, nested = keysets["admin"]
    ok("KL2 仪器自证：值换行的 cheatersNote 算叶子=%s、嵌套对象 navSubs "
       "算嵌套=%s" % ("cheatersNote" in leaves, "navSubs" in nested),
       "cheatersNote" in leaves and "navSubs" in nested,
       "行级取键会把前者漏成假红、把后者当缺失假红——两个坑都踩过")

    probe = sorted({a[1] for a in accesses if a[1] in leaves})[0]
    n = sum(1 for a in accesses if a[1] == probe)
    dropped = {k: (v[0] - {probe}, v[1]) for k, v in keysets.items()}
    rep = [a for a in accesses if a[1] not in dropped[a[0]][0]]
    ok("KL3 正控：抹掉在用的 %s（%d 处访问点）后，判据必须只报它"
       % (probe, n),
       n > 0 and len(rep) == n and all(a[1] == probe for a in rep),
       "报不出来＝KL1 不承重；报多了＝取值范围又错了")

    ok("KL4 基数：admin 段叶子 %d（>=120）" % len(leaves), len(leaves) >= 120,
       "后台文案被人整段删掉时，KL1 会因为「键集空」而假绿")

    summary()


if __name__ == "__main__":
    main()

"""一次性：i18n_baseline.json 键从反斜杠（Windows glob 产物）归一为正斜杠。

背景：glob 在 Windows 返回反斜杠、在 Linux/CI 返回正斜杠。基线一直按
Windows 风格记账，CI 消费时键全部错位 → 门禁在 CI 上形同虚设/全量误报
（2026-10-03 CI 实炸后定位）。i18n_guard 已改为键统一正斜杠；本脚本把
存量基线一次性搬过去，并校验搬完与 guard 的口径一致。
"""
import json
from pathlib import Path

P = Path("scripts/i18n_baseline.json")
d = json.loads(P.read_text(encoding="utf-8"))
fixed = {k.replace("\\", "/"): v for k, v in d.items()}
assert len(fixed) == len(d), "归一后键数变了，说明基线里本就有两套重复键"
P.write_text(json.dumps(fixed, indent=1, ensure_ascii=False) + "\n",
             encoding="utf-8")
print(f"normalized {len(fixed)} keys to forward-slash")

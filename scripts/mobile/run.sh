#!/bin/bash
# 模拟器 CDP 自愈入口: 确保 webview 浏览器活着 + devtools socket 已 forward, 再跑 node 脚本
# ADB/SER 可用环境变量覆盖（默认值适配本机开发环境）
ADB="${ADB:-/c/Users/52sy/AppData/Local/Android/Sdk/platform-tools/adb.exe}"
SER="${SER:-emulator-5554}"

# 0) WebView 必须在前台——被别的 app 盖住时 CDP eval 会挂死(无响应也不报错)
FOCUS=$("$ADB" -s $SER shell "dumpsys activity activities | grep topResumedActivity" | head -1)
if ! echo "$FOCUS" | grep -q webview_shell; then
  "$ADB" -s $SER shell "am force-stop com.kechengbiao.kechengbiao" >/dev/null 2>&1
  "$ADB" -s $SER shell am start -a android.intent.action.VIEW -d "http://localhost:3000/torrents" >/dev/null 2>&1
  sleep 4
fi

# 1) 浏览器进程不在就拉起
if ! "$ADB" -s $SER shell "pidof org.chromium.webview_shell" | grep -q .; then
  "$ADB" -s $SER shell am start -a android.intent.action.VIEW -d "http://localhost:3000/torrents" >/dev/null 2>&1
  sleep 4
fi

# 2) 找 devtools socket (PID 会变)
SOCK=$("$ADB" -s $SER shell "cat /proc/net/unix" | grep -o 'webview_devtools_remote_[0-9]*' | head -1)
if [ -z "$SOCK" ]; then
  # 再试一次: 有时 renderer 冷启动慢
  "$ADB" -s $SER shell "input keyevent KEYCODE_WAKEUP" >/dev/null 2>&1
  sleep 2
  SOCK=$("$ADB" -s $SER shell "cat /proc/net/unix" | grep -o 'webview_devtools_remote_[0-9]*' | head -1)
fi
if [ -z "$SOCK" ]; then echo "ERROR: no devtools socket" >&2; exit 1; fi

# 3) 重建 forward(旧 socket 名可能失效)
"$ADB" -s $SER forward --remove tcp:9222 >/dev/null 2>&1
"$ADB" -s $SER forward tcp:9222 localabstract:$SOCK

# 4) 确认 HTTP 探活
for i in 1 2 3; do
  if curl -s --max-time 4 http://127.0.0.1:9222/json/list | grep -q webSocketDebuggerUrl; then
    echo "CDP ready ($SOCK)" >&2
    exec node "$(dirname "$0")/cdp_bridge.mjs" "$@"
  fi
  sleep 2
done
echo "ERROR: CDP http not responding" >&2; exit 1

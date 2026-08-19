"""android_install_guard.py — vivo 安全守护确认框同步点击安装（spoq-fix M-E2E）

用户指示：adb 安装时不能等待卡死，必须同步抓 UI 树 + 点击通过 vivo 安全验证。

流程（一气呵成，任何一步有超时保护，绝不无限挂起）：
  1. push APK 到 /data/local/tmp/
  2. 后台发起 `pm install -r -t`（subprocess timeout 保护）
  3. 同步循环：uiautomator dump 抓 UI 树 → 解析"继续安装/安装/确定/允许"按钮 → input tap
     （dump 失败自动重启 uiautomator 服务重试，最多 N 轮）
  4. 完成判定：pm path 非空 且 lastUpdateTime 变化
"""
import subprocess
import re
import sys
import time
import xml.etree.ElementTree as ET

APK = r"Live2D-Ai-Android/app/build/outputs/apk/debug/app-debug.apk"
PKG = "com.live2d.ai.android"
DEVICE_TMP = "/data/local/tmp/live2d-ai.apk"
CONFIRM_KEYWORDS = ["继续安装", "安装", "确定", "允许", "仍要安装", "继续"]
DISCLAIMER_KEYWORDS = ["已了解", "风险检测结果", "风险"]
MAX_DUMP_RETRY = 3       # dump 失败重启 uiautomator 的轮数
MAX_CLICK_ROUNDS = 15    # 确认框检测轮数（每轮 ~3s）
INSTALL_TIMEOUT = 90     # pm install 超时（秒）


def sh(cmd, timeout=30):
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout,
                       encoding="utf-8", errors="replace")
    return r.stdout, r.stderr, r.returncode


def dump_ui():
    """抓 UI 树（vivo 限制 /sdcard 写 → dump 到 /data/local/tmp + cat 读取）；失败返回 None。"""
    out, _, _ = sh(["adb", "shell", "uiautomator", "dump", "/data/local/tmp/ui_guard.xml"], 25)
    if "dumped" not in out:
        return None
    cat, _, rc = sh(["adb", "shell", "cat", "/data/local/tmp/ui_guard.xml"], 20)
    if rc != 0 or not cat:
        return None
    return cat


def find_buttons(xml):
    """从 UI 树提取按钮文本 + 中心坐标。"""
    buttons = []
    try:
        root = ET.fromstring(xml)
        for node in root.iter("node"):
            text = node.get("text") or ""
            bounds = node.get("bounds") or ""
            if not text.strip() or not bounds.startswith("["):
                continue
            m = re.findall(r"\[(\d+),(\d+)\]\[(\d+),(\d+)\]", bounds)
            if not m:
                continue
            x1, y1, x2, y2 = map(int, m[0])
            buttons.append((text.strip(), (x1 + x2) // 2, (y1 + y2) // 2))
    except ET.ParseError:
        pass
    return buttons


def tap(x, y):
    sh(["adb", "shell", "input", "tap", str(x), str(y)], 15)


def install_apk():
    """主流程：push + 后台 install + 同步点击确认框。"""
    print("[1/4] push APK...")
    out, _, rc = sh(["adb", "push", APK, DEVICE_TMP], 60)
    print("  ", out.strip())

    print("[2/4] 后台发起 pm install（超时 %ds，不等待卡死）..." % INSTALL_TIMEOUT)
    proc = subprocess.Popen(
        ["adb", "shell", "pm", "install", "-r", "-t", DEVICE_TMP],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        encoding="utf-8", errors="replace", text=True,
    )

    old_time = None
    out2, _, _ = sh(["adb", "shell", "dumpsys", "package", PKG], 20)
    m = re.search(r"lastUpdateTime=([\d\- :]+)", out2 or "")
    if m:
        old_time = m.group(1).strip()
    print(f"  安装前 lastUpdateTime: {old_time}")

    print("[3/4] 同步轮询 UI 树点击确认框（vivo 安全守护）...")
    clicked = set()
    for rnd in range(1, MAX_CLICK_ROUNDS + 1):
        # 完成判定：进程已结束且时间变化
        if proc.poll() is not None:
            print(f"  install 进程结束（rc={proc.returncode}）")
            break
        xml = None
        for retry in range(MAX_DUMP_RETRY):
            xml = dump_ui()
            if xml:
                break
            # dump 失败：重启 uiautomator 服务
            sh(["adb", "shell", "pkill", "-f", "uiautomator"], 10)
            time.sleep(2)
        if not xml:
            print(f"  轮{rnd}: UI 树抓取失败（uiautomator 不可用），继续轮询")
            time.sleep(2)
            continue
        buttons = find_buttons(xml)
        # 先点"已了解风险"（勾选后才能点继续安装）
        for text, x, y in buttons:
            if any(k in text for k in DISCLAIMER_KEYWORDS) and (text, x, y) not in clicked:
                print(f"  轮{rnd}: 点勾选 [{text}] ({x},{y})")
                tap(x, y)
                clicked.add((text, x, y))
                time.sleep(1)
        for text, x, y in buttons:
            if any(k in text for k in CONFIRM_KEYWORDS) and (text, x, y) not in clicked:
                print(f"  轮{rnd}: 点确认 [{text}] ({x},{y})")
                tap(x, y)
                clicked.add((text, x, y))
        time.sleep(2)

    print("[4/4] 等待安装完成...")
    try:
        proc.wait(timeout=INSTALL_TIMEOUT)
    except subprocess.TimeoutExpired:
        proc.kill()
        print("  install 超时被终止（确认框未通过？检查设备屏幕）")
    out2, _, _ = sh(["adb", "shell", "dumpsys", "package", PKG], 20)
    m = re.search(r"lastUpdateTime=([\d\- :]+)", out2 or "")
    new_time = m.group(1).strip() if m else None
    print(f"  安装后 lastUpdateTime: {new_time}")
    ok = new_time and new_time != old_time
    print("安装结果:", "SUCCESS ✅" if ok else "未变化（可能失败/未装）")
    return ok


if __name__ == "__main__":
    install_apk()

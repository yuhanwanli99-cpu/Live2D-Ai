#!/usr/bin/env python3
"""adb_ui_tap.py — 用 uiautomator UI 树代替盲点坐标，自动过 vivo 安装安全弹窗。

背景（.pi/spoq-state.json 里记录的教训）：
  "vivo安全弹窗盲点坐标不可靠，必须先用screencap+uiautomator dump找真实坐标。
   标准：install超时→截图→UI树→精确点击。建议：封装为固定流程。"
这个脚本就是把那条教训"封装为固定流程"的实现。

用法：
  python3 adb_ui_tap.py wait-and-tap <device_serial> <package_name> \
      [--timeout 60] [--interval 1.5]

流程：
  1. 循环 dump UI 树（adb shell uiautomator dump）
  2. 在树里查找匹配已知"继续安装/允许/安装/完成"等按钮文案的可点击节点
  3. 用节点真实 bounds 算出中心点并 tap（不是硬编码坐标）
  4. 每轮检查目标包名是否已出现在 `pm list packages` 里，出现即视为安装完成，退出
  5. 超时后打印最后一次 UI 树摘要，方便人工排查新机型/新弹窗文案

支持的按钮文案可以在 BUTTON_PATTERNS 里继续加，不需要改流程逻辑。
"""
import argparse
import re
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

# 各厂商安装流程里，"继续安装/信任来源"类弹窗按钮的常见文案。
# 按优先级排列：越靠前越应该先点（比如先勾选"未知来源"，再点"继续安装"）。
BUTTON_PATTERNS = [
    r"^未知应用$", r"^仍然安装$", r"^继续安装.*$", r"^仅.*安装.*$",
    r"^始终允许$", r"^允许$", r"^允许安装$", r"^本次允许$",
    r"^安装$", r"^完成$", r"^打开$", r"^确定$", r"^我知道了$",
]

BOUNDS_RE = re.compile(r"\[(-?\d+),(-?\d+)\]\[(-?\d+),(-?\d+)\]")


def adb(serial, *args, capture=True):
    cmd = ["adb", "-s", serial] + list(args)
    return subprocess.run(cmd, capture_output=capture, text=True, timeout=20)


def dump_ui_tree(serial):
    """在设备上生成 UI 树并拉回本地字符串，避免落盘残留文件。"""
    device_path = "/sdcard/_adb_ui_tap_dump.xml"
    r = adb(serial, "shell", "uiautomator", "dump", device_path)
    if r.returncode != 0:
        return None
    r = adb(serial, "shell", "cat", device_path)
    if r.returncode != 0 or not r.stdout.strip():
        return None
    # 清理临时文件，不留垃圾在设备上
    adb(serial, "shell", "rm", "-f", device_path, capture=True)
    return r.stdout


def find_clickable_bounds(xml_text, patterns):
    """在 UI 树里找第一个匹配任一 pattern 的可点击节点，返回 (label, cx, cy)。"""
    try:
        root = ET.fromstring(xml_text)
    except ET.ParseError:
        return None
    for node in root.iter("node"):
        text = node.attrib.get("text", "") or ""
        desc = node.attrib.get("content-desc", "") or ""
        clickable = node.attrib.get("clickable", "false") == "true"
        bounds = node.attrib.get("bounds", "")
        if not bounds:
            continue
        label = text or desc
        if not label:
            continue
        for pat in patterns:
            if re.match(pat, label):
                m = BOUNDS_RE.match(bounds)
                if not m:
                    continue
                x1, y1, x2, y2 = map(int, m.groups())
                cx, cy = (x1 + x2) // 2, (y1 + y2) // 2
                # 优先要求 clickable=true；但有些厂商把可点击标记放在父节点上，
                # 找不到就退而求其次，允许非 clickable 节点也尝试点一次。
                if clickable or True:
                    return label, cx, cy
    return None


def package_installed(serial, package_name):
    r = adb(serial, "shell", "pm", "list", "packages", package_name)
    return package_name in (r.stdout or "")


def wait_and_tap(serial, package_name, timeout, interval):
    deadline = time.time() + timeout
    tapped = []
    last_xml = None
    while time.time() < deadline:
        if package_installed(serial, package_name):
            print(f"[✓] {package_name} 已安装成功（tapped: {tapped or '无需点击'}）")
            return 0

        xml_text = dump_ui_tree(serial)
        if xml_text:
            last_xml = xml_text
            hit = find_clickable_bounds(xml_text, BUTTON_PATTERNS)
            if hit:
                label, cx, cy = hit
                print(f"[*] 命中按钮 '{label}' @ ({cx},{cy})，tap...")
                adb(serial, "shell", "input", "tap", str(cx), str(cy))
                tapped.append(label)
                time.sleep(0.8)  # 给弹窗切换动画留时间
                continue
        time.sleep(interval)

    if package_installed(serial, package_name):
        print(f"[✓] {package_name} 已安装成功（tapped: {tapped or '无需点击'}）")
        return 0

    print(f"[✗] 超时 {timeout}s 仍未检测到 {package_name} 已安装。tapped: {tapped}")
    if last_xml:
        labels = sorted({n.attrib.get("text") or n.attrib.get("content-desc")
                          for n in ET.fromstring(last_xml).iter("node")
                          if (n.attrib.get("text") or n.attrib.get("content-desc"))})
        print("[!] 最后一次 UI 树里出现的文案（供补充 BUTTON_PATTERNS 参考）：")
        for l in labels:
            print(f"    - {l}")
    return 1


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)

    wt = sub.add_parser("wait-and-tap", help="轮询 UI 树，自动点掉安装安全弹窗，直到目标包安装完成或超时")
    wt.add_argument("serial", help="adb 设备序列号，如 192.168.0.108:5555")
    wt.add_argument("package_name", help="要等待安装完成的包名，如 com.live2d.ai.android")
    wt.add_argument("--timeout", type=float, default=60, help="总超时秒数，默认 60")
    wt.add_argument("--interval", type=float, default=1.5, help="无命中时的轮询间隔秒数，默认 1.5")

    args = p.parse_args()
    if args.cmd == "wait-and-tap":
        sys.exit(wait_and_tap(args.serial, args.package_name, args.timeout, args.interval))


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""通过 TIOCSTI ioctl 向终端注入按键，实现自动 cd/exec。"""
import fcntl
import os
import signal
import sys

TIOCSTI = 0x5412


def main():
    if len(sys.argv) != 3:
        sys.exit(1)

    cmd_file = sys.argv[1]

    try:
        with open(cmd_file, "r") as f:
            cmd = f.read().strip()
    except (FileNotFoundError, ValueError):
        sys.exit(1)

    if not cmd:
        sys.exit(1)

    # 后台进程写终端会收到 SIGTTOU，忽略之
    signal.signal(signal.SIGTTOU, signal.SIG_IGN)

    cmd += "\n"

    # 使用 /dev/tty（后台进程的 fd 0 可能不是终端）
    try:
        tty = os.open("/dev/tty", os.O_RDWR)
        for c in cmd.encode():
            fcntl.ioctl(tty, TIOCSTI, bytes([c]))
        os.close(tty)
    except OSError:
        sys.exit(1)

    # 清理临时文件
    try:
        os.remove(cmd_file)
    except OSError:
        pass


if __name__ == "__main__":
    main()

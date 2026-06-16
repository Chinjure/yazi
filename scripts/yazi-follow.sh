#!/usr/bin/env bash
# yazi-follow.sh — 实时跟随 yazi CWD 并执行远程命令（无需按键）

yazi-follow() {
	yazi-unfollow 2>/dev/null

	_YAZI_FIFO="/tmp/yazi-follow-$$"
	mkfifo "$_YAZI_FIFO" 2>/dev/null || { echo "yazi-follow: 无法创建 FIFO"; return 1; }

	ya follow > "$_YAZI_FIFO" &
	_YAZI_FOLLOW_PID=$!

	{
		while IFS= read -r line < "$_YAZI_FIFO"; do
			[[ -z "$line" ]] && continue

			if [[ "$line" == EXEC$'\t'* ]]; then
				_YAZI_CWD="${line#EXEC$'\t'}"
				_YAZI_CMD="${_YAZI_CWD#*$'\t'}"
				_YAZI_CWD="${_YAZI_CWD%%$'\t'*}"
				printf '%s\n' "cd $(printf '%q' "$_YAZI_CWD") && $_YAZI_CMD" > "/tmp/yazi-follow-cmd-$$"
			else
				printf '%s\n' "cd $(printf '%q' "$line")" > "/tmp/yazi-follow-cmd-$$"
			fi

			python3 /home/user/yazi/scripts/yazi-follow-tiocsti.py "/tmp/yazi-follow-cmd-$$" "$$" 2>/dev/null
		done
	} &
	_YAZI_FOLLOW_READER=$!

	echo "yazi-follow: 已开始实时跟随 yazi。运行 'yazi-unfollow' 停止。"
}

yazi-unfollow() {
	[[ -n "$_YAZI_FOLLOW_PID" ]] && kill "$_YAZI_FOLLOW_PID" 2>/dev/null
	[[ -n "$_YAZI_FOLLOW_READER" ]] && kill "$_YAZI_FOLLOW_READER" 2>/dev/null
	rm -f "/tmp/yazi-follow-$$" "/tmp/yazi-follow-cmd-$$" 2>/dev/null
	unset _YAZI_FOLLOW_PID _YAZI_FOLLOW_READER _YAZI_FIFO
	echo "yazi-follow: 已停止。"
}

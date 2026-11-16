#!/bin/sh
brief="$1"

extract() {
    printf '%s' "$brief" | sed -n "s/.*\\[$1:\\([^]]*\\)\\].*/\\1/p"
}

has() {
    case "$brief" in
        *"[$1]"*) return 0 ;;
        *) return 1 ;;
    esac
}

if has crash; then
    exit 3
fi

write_arg=$(extract write)
if [ -n "$write_arg" ]; then
    write_path=${write_arg%%=*}
    write_text=${write_arg#*=}
    printf '%s\n' "$write_text" > "$write_path"
fi

delay_ms=$(extract delay)
[ -z "$delay_ms" ] && delay_ms=100
sec=$((delay_ms / 1000))
ms=$((delay_ms % 1000))
sleep "$(printf '%d.%03d' "$sec" "$ms")" &
pid=$!

steers=""
while kill -0 "$pid" 2>/dev/null; do
    if read -t 0.2 line; then
        steers="$steers $line"
    fi
done
if read -t 0.2 line; then
    steers="$steers $line"
fi

stop_q=$(extract stop)
if [ -n "$stop_q" ]; then
    printf 'stop:%s\n' "$stop_q"
    read -r line
    steers="$steers $line"
fi

read_suffix=""
read_path=$(extract read)
if [ -n "$read_path" ]; then
    if cat "$read_path" >/dev/null 2>&1; then
        read_suffix=" read:${read_path}=ok"
    else
        read_suffix=" read:${read_path}=fail"
    fi
fi

probe_suffix=""
if has probe; then
    names=$(env | cut -d= -f1 | sort | tr '\n' ',' | sed 's/,$//')
    home_entries=$(ls -A "$HOME" 2>/dev/null | wc -l)
    probe_suffix=" env=$names home=$HOME home_entries=$home_entries cwd=$(pwd)"
fi

verdict_override=$(extract verdict)
base=${verdict_override:-"done: $brief"}
printf 'verdict:%s\n' "$base$steers$read_suffix$probe_suffix"

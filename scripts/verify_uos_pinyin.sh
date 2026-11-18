#!/bin/sh
# Run the three desktop IME checks with a private Fcitx5 on the actual logged-in X11 display.
set -eu
if [ "$#" -ne 2 ]; then echo 'usage: verify_uos_pinyin.sh CARGO_DEBUG_DIR NEW_OUTPUT_DIR' >&2; exit 2; fi
: "${DISPLAY:?use the logged-in X11 display}"
bin=$(cd "$1" && pwd)
scripts=$(cd "$(dirname "$0")" && pwd)
mkdir "$2"
out=$(cd "$2" && pwd)
profile=$(mktemp -d)
cleanup() {
    # The private session bus may exit just before its GVfs/portal FUSE mounts.
    # Wait for those mounts to disappear before removing their parent directory.
    for attempt in $(seq 1 50); do
        if ! mountpoint -q -- "$profile/runtime/gvfs" && ! mountpoint -q -- "$profile/runtime/doc"; then
            if rm -rf --one-file-system -- "$profile"; then return 0; fi
        fi
        sleep .2
    done
    echo "Temporary input-method profile still in use: $profile" >&2
    return 1
}
trap cleanup EXIT
export XDG_CONFIG_HOME="$profile/config" XDG_DATA_HOME="$profile/data" XDG_CACHE_HOME="$profile/cache" XDG_RUNTIME_DIR="$profile/runtime" XDG_STATE_HOME="$profile/state"
mkdir -p "$XDG_CONFIG_HOME/fcitx5/conf" "$XDG_DATA_HOME" "$XDG_CACHE_HOME" "$XDG_RUNTIME_DIR" "$XDG_STATE_HOME"
chmod 700 "$XDG_RUNTIME_DIR"
export XMODIFIERS="@im=joyoffice-qa-$$" GTK_IM_MODULE=fcitx QT_IM_MODULE=fcitx
unset DBUS_SESSION_BUS_ADDRESS WAYLAND_DISPLAY
cat > "$XDG_CONFIG_HOME/fcitx5/profile" <<'PROFILE'
[Groups/0]
Name=JoyOffice
Default Layout=us
DefaultIM=pinyin

[Groups/0/Items/0]
Name=keyboard-us

[Groups/0/Items/1]
Name=pinyin

[GroupOrder]
0=JoyOffice
PROFILE
echo UseOnTheSpot=True > "$XDG_CONFIG_HOME/fcitx5/conf/xim.conf"
dbus-run-session -- sh -eu -c '
    fcitx5 -D -u classicui --disable all --enable keyboard,classicui,xcb,xim,dbus,pinyin,punctuation > "$3/fcitx.log" 2>&1 &
    daemon=$!
    trap "kill $daemon 2>/dev/null || true" EXIT
    for attempt in $(seq 1 60); do
        fcitx5-remote -n >/dev/null 2>&1 && break
        sleep 0.2
    done
    python3 "$1/probe_native_ime.py" "$2/examples/ime_probe" "$3/probe" --method pinyin
    for product in word deck sheet; do
        python3 "$1/verify_uos_native_ime.py" "$product" "$2/joy$product" "$3/$product"
    done
' sh "$scripts" "$bin" "$out"

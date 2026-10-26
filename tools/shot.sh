#!/usr/bin/env bash
# Screenshot the GTK editor on a private X server (Xvfb :77).
#
#   tools/shot.sh OUT.png [FILE.monopic] [-- xdotool args ...]
#
# Env: BIN (default target/debug/solodraw-gtk), SIZE (default 1280x820), WAIT (seconds, default 2).
# Steps after "--" are passed to xdotool one command per ';'-separated group, e.g.
#   tools/shot.sh out.png fixtures/x.monopic -- key r ';' mousemove 400 300 ';' click 1
set -euo pipefail
cd "$(dirname "$0")/.."

out=${1:?usage: shot.sh OUT.png [FILE] [-- xdotool steps]}
shift
file=""
if [[ $# -gt 0 && $1 != "--" ]]; then file=$1; shift; fi
[[ ${1:-} == "--" ]] && shift

BIN=${BIN:-target/debug/solodraw-gtk}
SIZE=${SIZE:-1280x820}
WAIT=${WAIT:-2}
export DISPLAY=:77
# Keep test runs out of the user's recent files; reuse the installed FIGlet fonts.
export XDG_DATA_HOME=${SHOT_DATA:-/tmp/solodraw-shot-data}
mkdir -p "$XDG_DATA_HOME"
[[ -e $XDG_DATA_HOME/solodraw || ! -d $HOME/.local/share/solodraw ]] || ln -s "$HOME/.local/share/solodraw" "$XDG_DATA_HOME/solodraw"

if ! xdpyinfo -display :77 >/dev/null 2>&1; then
    Xvfb :77 -screen 0 "${SIZE}x24" -nolisten tcp >/dev/null 2>&1 &
    for _ in $(seq 50); do xdpyinfo -display :77 >/dev/null 2>&1 && break; sleep 0.1; done
fi

GDK_BACKEND=x11 GTK_A11Y=none NO_AT_BRIDGE=1 "$BIN" ${file:+"$file"} >/tmp/solodraw-shot.log 2>&1 &
app=$!
trap 'kill $app 2>/dev/null || true' EXIT
sleep "$WAIT"

if [[ $# -gt 0 ]]; then
    win=$(xdotool search --sync --onlyvisible --pid "$app" | head -1)
    xdotool windowactivate --sync "$win" 2>/dev/null || xdotool windowfocus "$win"
    step=()
    for a in "$@" ";"; do
        if [[ $a == ";" ]]; then
            [[ ${#step[@]} -gt 0 ]] && xdotool "${step[@]}" && sleep 0.3
            step=()
        else
            step+=("$a")
        fi
    done
    sleep 0.5
fi

import -window root "$out"
echo "$out"

#!/usr/bin/env bash
# Takes the screenshots in this folder. Needs sway, grim, jq and wtype.
#
# Each shot runs a fresh rechner with its own config directory and its own
# D-Bus session, so an already open rechner neither answers the launch nor
# gets its settings changed.

set -euo pipefail

cd "$(dirname "$0")/.."
cargo build --release --quiet
bin=$PWD/target/release/rechner
out=$PWD/examples
cfg=$(mktemp -d)
trap 'rm -rf "$cfg"' EXIT

# Geometry of the focused rechner window, empty if another window has focus.
focused_rechner() {
    swaymsg -t get_tree | jq -r '
        .. | objects | select(.app_id? == "io.github.HollowPhobos.rechner" and .focused)
        | "\(.rect.x + .window_rect.x),\(.rect.y + .window_rect.y) \(.window_rect.width)x\(.window_rect.height)"' |
        head -n1
}

# wtype sends keys to whatever has focus, so stop before typing anywhere else.
type_into_rechner() {
    [[ -n $(focused_rechner) ]] || { echo "rechner lost focus, stopping" >&2; kill "$session"; exit 1; }
    wtype -s 300 "$@"
}

# shot <file> <theme> <lang> <notation> <keys>
#   <keys> is typed as text; a trailing "\n" presses Enter to confirm the result.
#   With file names as arguments, only those shots are taken.
shot() {
    local file=$1 theme=$2 lang=$3 notation=$4 keys=$5
    if ((${#only[@]})) && [[ ! " ${only[*]} " =~ " $file " ]]; then
        return
    fi
    mkdir -p "$cfg/rechner"
    printf 'theme=%s\nlang=%s\nnotation=%s\nopacity=100\n' "$theme" "$lang" "$notation" \
        >"$cfg/rechner/config"

    XDG_CONFIG_HOME=$cfg dbus-run-session -- "$bin" 2>/dev/null &
    session=$!

    for _ in $(seq 50); do
        sleep 0.1
        [[ -n $(focused_rechner) ]] && break
    done

    # wtype uploads its own keymap first; keys sent before the window has
    # picked it up are dropped.
    sleep 0.3
    local text=${keys%\\n}
    type_into_rechner -d 20 "$text"
    [[ $text != "$keys" ]] && type_into_rechner -k Return
    sleep 0.3

    local geometry
    geometry=$(focused_rechner)
    [[ -n $geometry ]] || { echo "rechner lost focus, stopping" >&2; kill "$session"; exit 1; }
    grim -g "$geometry" "$out/$file"
    kill "$session"
    wait "$session" 2>/dev/null || true
    echo "$file"
}

only=("$@")
shot light.png            light            en comma '1.234,5×(2+3)'
shot percent.png          gruvbox-light    en comma '200+10%\n'
shot catppuccin-mocha.png catppuccin-mocha en comma '0,1+0,2\n'
shot german.png           nord             de comma '√(9+16)×2\n'
shot decimal-point.png    dark             en point '1,234.5×(2+3)\n'

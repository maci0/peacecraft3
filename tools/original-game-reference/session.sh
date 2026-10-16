#!/bin/sh
set -eu
mkdir -p "$HOME" captures "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
mkdir -p "$HOME/.config/pulse"
printf "enable-shm = no\nenable-memfd = no\n" >"$HOME/.config/pulse/client.conf"
export PULSE_SERVER="unix:$XDG_RUNTIME_DIR/native"
pulseaudio --daemonize=yes --exit-idle-time=-1 --log-target=file:/run/reference/pulseaudio.log -n -L "module-native-protocol-unix socket=$XDG_RUNTIME_DIR/native" -L "module-null-sink sink_name=reference"
pactl list short sinks >audio-sinks.txt
Xvfb "$DISPLAY" -screen 0 "${REFERENCE_WIDTH}x${REFERENCE_HEIGHT}x24" -dpi "$REFERENCE_DPI" -r -nolisten tcp >xvfb.log 2>&1 &
xpid=$!
trap 'wineserver -k || true; pulseaudio --kill || true; kill "$xpid" || true' EXIT INT TERM
for i in $(seq 1 100); do xdpyinfo >/dev/null 2>&1 && break; sleep .1; done
xdpyinfo >display.txt
glxinfo -B >renderer.txt 2>&1
wine --version >wine-version.txt
dpkg-query -W >packages.txt
wineboot --init >wineboot.log 2>&1
wineserver -w
wine reg query 'HKCU\Software\Blizzard Entertainment\Warcraft III\Video' >video-before.txt 2>&1 || true
printf '%s\n' ready >ready
cd game
set +e
WINEDLLOVERRIDES="$WINEDLLOVERRIDES;winegstreamer=d" wine explorer "/desktop=Reference,${REFERENCE_WIDTH}x${REFERENCE_HEIGHT}" 'Z:\run\reference\game\Warcraft III.exe' "$@" >../game.log 2>&1
code=$?
printf '%s\n' "$code" >../launcher-exit-code.txt
wineserver -w

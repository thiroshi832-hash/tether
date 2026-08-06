#!/bin/sh
# Load or unload the Tether virtual-microphone PulseAudio modules.
#
# Runs from the systemd --user service tether-vmic.service on login; the Rust
# side (src/virtual_mic/linux.rs) also invokes it on-demand from the running
# app for users who don't have systemd --user.

set -e

SINK_NAME="tether_audio"
SOURCE_NAME="tether_microphone"
SINK_DESC="Tether_Audio"
SOURCE_DESC="Tether_Microphone"

case "${1:-load}" in
    load)
        if ! pactl list short sinks 2>/dev/null | awk '{print $2}' | grep -qx "$SINK_NAME"; then
            pactl load-module module-null-sink \
                sink_name="$SINK_NAME" \
                sink_properties="device.description=$SINK_DESC" >/dev/null
        fi
        if ! pactl list short sources 2>/dev/null | awk '{print $2}' | grep -qx "$SOURCE_NAME"; then
            pactl load-module module-remap-source \
                source_name="$SOURCE_NAME" \
                master="$SINK_NAME.monitor" \
                source_properties="device.description=$SOURCE_DESC" >/dev/null
        fi
        ;;
    unload)
        # Unload any modules that mention our sink/source name.
        pactl list short modules 2>/dev/null | \
            awk -v s="$SINK_NAME" -v r="$SOURCE_NAME" \
                '$0 ~ ("sink_name=" s) || $0 ~ ("source_name=" r) {print $1}' | \
            while read -r idx; do
                pactl unload-module "$idx" >/dev/null 2>&1 || true
            done
        ;;
    *)
        echo "usage: $0 {load|unload}" >&2
        exit 2
        ;;
esac

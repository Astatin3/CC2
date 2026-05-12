#!/bin/sh
set -eu

REMOTEPROC_STATE=/sys/class/remoteproc/remoteproc0/state
CTRL_DEV=/dev/rpmsg_ctrl-dsp_rproc@0
REMOTE_NAME=dsp_rproc@0
ENDPOINT_NAME=cpu_dsp0
LOG=/tmp/start-rpmsg.log
RESTART=1
RUN_IDENTIFY=0

usage() {
    cat <<EOF
Usage: ./start-rpmsg.sh [--no-restart] [--run-identify]

Starts the DSP remoteproc, creates Elegoo's Klipper/RPMsg endpoint named cpu_dsp0,
and prints the resulting /dev/rpmsgN device.

Options:
  --no-restart   Do not stop/start remoteproc0 first.
  --run-identify Run ./klippy-test against the created endpoint.

Make sure elegoo_printer is stopped before running this.
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --no-restart)
            RESTART=0
            ;;
        --run-identify)
            RUN_IDENTIFY=1
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
    shift
done

if pidof elegoo_printer >/dev/null 2>&1; then
    echo "elegoo_printer is running. Stop it before creating a standalone RPMsg endpoint." >&2
    exit 1
fi

if [ ! -w "$REMOTEPROC_STATE" ]; then
    echo "missing or non-writable remoteproc state: $REMOTEPROC_STATE" >&2
    exit 1
fi

if ! command -v rpmsg_test >/dev/null 2>&1; then
    echo "missing rpmsg_test utility" >&2
    exit 1
fi

if [ "$RESTART" -eq 1 ]; then
    echo "restarting DSP remoteproc0"
    echo stop > "$REMOTEPROC_STATE" 2>/dev/null || true
    sleep 1
    echo start > "$REMOTEPROC_STATE"
    sleep 1
else
    echo "leaving remoteproc0 state unchanged"
fi

deadline=$(( $(date +%s) + 8 ))
while [ ! -e "$CTRL_DEV" ]; do
    if [ "$(date +%s)" -ge "$deadline" ]; then
        echo "timed out waiting for $CTRL_DEV" >&2
        exit 1
    fi
    sleep 1
done

before="$(ls /dev/rpmsg[0-9]* 2>/dev/null || true)"

echo "creating endpoint '$ENDPOINT_NAME' through $CTRL_DEV"
rm -f "$LOG"
rpmsg_test -r "$REMOTE_NAME" -c "$ENDPOINT_NAME" >"$LOG" 2>&1 &
rpmsg_pid=$!

created=""
deadline=$(( $(date +%s) + 8 ))
while [ -z "$created" ]; do
    current="$(ls /dev/rpmsg[0-9]* 2>/dev/null || true)"
    for dev in $current; do
        found_before=0
        for old in $before; do
            if [ "$dev" = "$old" ]; then
                found_before=1
                break
            fi
        done
        if [ "$found_before" -eq 0 ]; then
            created="$dev"
            break
        fi
    done

    if [ -n "$created" ]; then
        break
    fi

    if ! kill -0 "$rpmsg_pid" 2>/dev/null; then
        echo "rpmsg_test exited before creating an endpoint" >&2
        cat "$LOG" >&2 2>/dev/null || true
        exit 1
    fi

    if [ "$(date +%s)" -ge "$deadline" ]; then
        echo "timed out waiting for /dev/rpmsgN endpoint" >&2
        kill "$rpmsg_pid" 2>/dev/null || true
        cat "$LOG" >&2 2>/dev/null || true
        exit 1
    fi

    sleep 1
done

kill "$rpmsg_pid" 2>/dev/null || true
sleep 1

if [ ! -e "$created" ]; then
    fallback="$(ls /dev/rpmsg[0-9]* 2>/dev/null | tail -n 1 || true)"
    if [ -n "$fallback" ]; then
        created="$fallback"
    fi
fi

if [ ! -e "$created" ]; then
    echo "endpoint disappeared after rpmsg_test exited" >&2
    cat "$LOG" >&2 2>/dev/null || true
    exit 1
fi

echo "created_endpoint=$created"
echo "remoteproc_state=$(cat "$REMOTEPROC_STATE" 2>/dev/null || true)"
echo "rpmsg_devices:"
ls -l /dev/rpmsg* 2>/dev/null || true

if [ "$RUN_IDENTIFY" -eq 1 ]; then
    if [ ! -x ./klippy-test ]; then
        echo "./klippy-test not found or not executable" >&2
        exit 1
    fi
    exec ./klippy-test --device "$created" --drain-ms 0 --timeout-ms 5000
fi

echo "next: ./klippy-test --device $created --drain-ms 0 --timeout-ms 5000"

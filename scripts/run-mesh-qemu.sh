#!/usr/bin/env bash
set -euo pipefail
# Mocked mesh gates (PRD v2 D13). QEMU flags must stay in sync with run-qemu.sh.
ELF="${1:?usage: run-mesh-qemu.sh <elf> [log] [mode] [port]}"
LOG="${2:-/tmp/apexos-rv-mesh.log}"
MODE="${3:-echo}"
PORT="${4:-9601}"

case "$MODE" in
  echo)   MARKER="net: tcp echo ok";               EXPECT=0 ;;
  ws)     MARKER="mesh: session";                  EXPECT=0 ;;
  llm)    MARKER="APEXOS-RV: goal done — halting"; EXPECT=0 ;;
  silent) MARKER="step stalled — no completion";   EXPECT=2 ;;
  *)      echo "unknown mode $MODE" >&2; exit 2 ;;
esac

cargo build -q -p apexos-rv-xtest --bin mockd --target x86_64-unknown-linux-gnu
MOCK="target/x86_64-unknown-linux-gnu/debug/mockd"
"$MOCK" "$MODE" "$PORT" &
MPID=$!
trap 'kill "$MPID" 2>/dev/null || true' EXIT
sleep 0.3

set +e
timeout 30s qemu-system-riscv64 -machine virt -cpu rv64 -smp 1 -m 256M \
  -bios none -nographic -serial mon:stdio \
  -global virtio-mmio.force-legacy=false \
  -netdev user,id=n0 -device virtio-net-device,netdev=n0,mac=52:54:00:0b:ee:f1 \
  -kernel "$ELF" | tee "$LOG"
RC=${PIPESTATUS[0]}
set -e
grep -q "apexos-rv: hart 0 online" "$LOG"
grep -q "$MARKER" "$LOG"
[ "$RC" -eq "$EXPECT" ] || { echo "exit $RC != expected $EXPECT" >&2; exit 1; }

#!/usr/bin/env bash
set -euo pipefail
# Live-colony demo (PRD v2 DoD — non-normative evidence, never a CI gate).
# The kernel reaches the LAN through QEMU slirp; point it at a real agentd.
# agentd on THIS host: use the slirp host alias 10.0.2.2 + the real port.
#
#   APEXRV_GATEWAY_IP=10.0.2.2 APEXRV_GATEWAY_PORT=8080 \
#   APEXRV_TOKEN=<bearer-if-set> ./scripts/run-live.sh
#
: "${APEXRV_GATEWAY_IP:?set APEXRV_GATEWAY_IP (agentd address; 10.0.2.2 = this host)}"
: "${APEXRV_GATEWAY_PORT:?set APEXRV_GATEWAY_PORT (agentd port)}"
export APEXRV_GATEWAY_IP APEXRV_GATEWAY_PORT
export APEXRV_STEP_TIMEOUT_SECS="${APEXRV_STEP_TIMEOUT_SECS:-120}"
[ -n "${APEXRV_TOKEN:-}" ] && export APEXRV_TOKEN

LOG="${1:-docs/live-run-$(date +%F).log}"
echo "live: building mesh-goal kernel → ws://${APEXRV_GATEWAY_IP}:${APEXRV_GATEWAY_PORT}/ws (step timeout ${APEXRV_STEP_TIMEOUT_SECS}s)"
cargo build --release --features mesh-goal

set +e
timeout 600s qemu-system-riscv64 -machine virt -cpu rv64 -smp 1 -m 256M \
  -bios none -nographic -serial mon:stdio \
  -global virtio-mmio.force-legacy=false \
  -netdev user,id=n0 -device virtio-net-device,netdev=n0,mac=52:54:00:0b:ee:f1 \
  -kernel target/riscv64gc-unknown-none-elf/release/apexos-rv-kernel | tee "$LOG"
RC=${PIPESTATUS[0]}
set -e
echo "live: kernel exited $RC — transcript in $LOG"

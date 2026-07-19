#!/usr/bin/env bash
set -euo pipefail
ELF="${1:?usage: run-qemu.sh <path-to-elf> [log]}"
LOG="${2:-/tmp/apexos-rv-uart.log}"
timeout 30s qemu-system-riscv64 -machine virt -cpu rv64 -smp 1 -m 256M \
  -bios none -nographic -serial mon:stdio -kernel "$ELF" | tee "$LOG"
# timeout kills a hang (exit 124); sifive_test propagates pass/fail otherwise
grep -q "apexos-rv: hart 0 online" "$LOG"
grep -q "APEXOS-RV: goal done — halting" "$LOG"

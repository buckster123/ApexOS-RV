MEMORY
{
  RAM : ORIGIN = 0x80000000, LENGTH = 128M
}

REGION_ALIAS("REGION_TEXT",   RAM);
REGION_ALIAS("REGION_RODATA", RAM);
REGION_ALIAS("REGION_DATA",   RAM);
REGION_ALIAS("REGION_BSS",    RAM);
REGION_ALIAS("REGION_HEAP",   RAM);
REGION_ALIAS("REGION_STACK",  RAM);

/* riscv-rt symbols — names/semantics: confirm against the pinned 0.18 docs */
_heap_size = 0x100000;        /* 1 MiB, if using the linker-provided heap route  */
/* _max_hart_id = 0;             default; raise only when -smp > 1 (Phase 7+)    */
/* _hart_stack_size = 0x10000;   per-hart stack if the default proves too small  */

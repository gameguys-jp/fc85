/* fc85.ld：C の ROM のリンカの設定（DR-103。説明は sdk/docs/c.md §03）。
 *
 * カートリッジの窓 $400000〜$4FFFFF（1 MiB）に、見出し（FC85_CART）、入口 _start（crt0.S）、コード、
 * 定数、.data の初期値を順に置く。.data と .bss は Main RAM のアプリの領域 APP_RAM（$100800）から置き、
 * スタックは INITIAL_SSP（$120000）から下へ。番地は sdk/include/fc85.h と sdk/c/include/bios.h と同じ。
 * スタックの大きさは既定 4 KiB（-Wl,--defsym=__stack_size=… で替える）。足りなければリンクで止まる。
 */
OUTPUT_FORMAT("elf32-m68k")
OUTPUT_ARCH(m68k)
ENTRY(_start)

MEMORY
{
  ROM (rx)  : ORIGIN = 0x400000, LENGTH = 0x100000
  RAM (rwx) : ORIGIN = 0x100800, LENGTH = 0x01F800
}

__stack_size = DEFINED(__stack_size) ? __stack_size : 0x1000;

SECTIONS
{
  .text :
  {
    KEEP(*(.cart_header))
    KEEP(*(.text.start))
    *(.text .text.*)
    *(.rodata .rodata.*)
    . = ALIGN(2);
  } > ROM

  .data :
  {
    __data_start = .;
    *(.data .data.*)
    . = ALIGN(2);
    __data_end = .;
  } > RAM AT > ROM
  __data_load = LOADADDR(.data);

  .bss (NOLOAD) :
  {
    __bss_start = .;
    *(.bss .bss.*)
    *(COMMON)
    . = ALIGN(2);
    __bss_end = .;
  } > RAM

  __rom_end = __data_load + SIZEOF(.data);
  __rom_size = __rom_end - ORIGIN(ROM);
  __stack_top = ORIGIN(RAM) + LENGTH(RAM);
  __heap_start = __bss_end;
  __heap_end = __stack_top - __stack_size;

  ASSERT(__bss_end <= __heap_end, "Main RAM が足りない（.data と .bss とスタック。sdk/docs/c.md §03）")
  ASSERT(DEFINED(fc85_cart_header), "見出しがない（FC85_CART を 1 つの .c に書く。sdk/docs/c.md §04）")
  ASSERT(_start == ORIGIN(ROM) + 32, "見出しが窓の先頭にない（FC85_CART は 1 つだけ）")

  /DISCARD/ : { *(.comment) *(.note .note.*) *(.eh_frame .eh_frame_hdr) *(.init_array* .fini_array* .ctors* .dtors*) }

  /* 以下はデバッグの情報（ROM に入らない）。GNU ld の既定の並び */
  .stab           0 : { *(.stab) }
  .stabstr        0 : { *(.stabstr) }
  .debug          0 : { *(.debug) }
  .line           0 : { *(.line) }
  .debug_srcinfo  0 : { *(.debug_srcinfo) }
  .debug_sfnames  0 : { *(.debug_sfnames) }
  .debug_aranges  0 : { *(.debug_aranges) }
  .debug_pubnames 0 : { *(.debug_pubnames) }
  .debug_info     0 : { *(.debug_info .gnu.linkonce.wi.*) }
  .debug_abbrev   0 : { *(.debug_abbrev) }
  .debug_line     0 : { *(.debug_line .debug_line.* .debug_line_end) }
  .debug_frame    0 : { *(.debug_frame) }
  .debug_str      0 : { *(.debug_str) }
  .debug_loc      0 : { *(.debug_loc) }
  .debug_macinfo  0 : { *(.debug_macinfo) }
  .debug_pubtypes 0 : { *(.debug_pubtypes) }
  .debug_ranges   0 : { *(.debug_ranges) }
  .debug_addr     0 : { *(.debug_addr) }
  .debug_line_str 0 : { *(.debug_line_str) }
  .debug_loclists 0 : { *(.debug_loclists) }
  .debug_macro    0 : { *(.debug_macro) }
  .debug_names    0 : { *(.debug_names) }
  .debug_rnglists 0 : { *(.debug_rnglists) }
  .debug_str_offsets 0 : { *(.debug_str_offsets) }
  .debug_sup      0 : { *(.debug_sup) }
  .gnu.attributes 0 : { KEEP(*(.gnu.attributes)) }
}

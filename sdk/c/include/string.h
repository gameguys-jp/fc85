/* string.h：C の ROM の小さな文字列とメモリの関数（DR-103。sdk/docs/c.md §05）。中身は sdk/c/string.c。
 * C の標準ライブラリの一部だけ。GCC は構造体の写しや配列の 0 の初期化で memcpy・memset を呼ぶことがある。
 * Main RAM と Staging RAM のほかの番地（MMIO の窓など）には使わない（幅と回数が決まらない）。
 */
#ifndef FC85_STRING_H
#define FC85_STRING_H

#include <stddef.h>

void *memcpy(void *restrict dst, const void *restrict src, size_t n);
void *memmove(void *dst, const void *src, size_t n);
void *memset(void *dst, int c, size_t n);
int memcmp(const void *a, const void *b, size_t n);
size_t strlen(const char *s);
int strcmp(const char *a, const char *b);
char *strcpy(char *restrict dst, const char *restrict src);

#endif

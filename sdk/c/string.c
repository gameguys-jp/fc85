/* string.c：string.h の関数（DR-103）。両方が偶数番地なら長語ずつ写す。
 * GCC がこの中の繰返しを memcpy・memset の呼出しに替えない（自分を呼ぶことになる）よう、その最適化を切る。
 */
#include <stdint.h>
#include <string.h>

#define NO_LIBCALL __attribute__((optimize("no-tree-loop-distribute-patterns")))

NO_LIBCALL void *memcpy(void *restrict dst, const void *restrict src, size_t n)
{
    uint8_t *d = dst;
    const uint8_t *s = src;
    if ((((uintptr_t)d | (uintptr_t)s) & 1) == 0) {
        uint32_t *dl = (uint32_t *)d;
        const uint32_t *sl = (const uint32_t *)s;
        for (; n >= 4; n -= 4)
            *dl++ = *sl++;
        d = (uint8_t *)dl;
        s = (const uint8_t *)sl;
    }
    while (n--)
        *d++ = *s++;
    return dst;
}

NO_LIBCALL void *memmove(void *dst, const void *src, size_t n)
{
    uint8_t *d = dst;
    const uint8_t *s = src;
    if (d <= s || d >= s + n)
        return memcpy(dst, src, n);
    d += n;
    s += n;
    while (n--)
        *--d = *--s;
    return dst;
}

NO_LIBCALL void *memset(void *dst, int c, size_t n)
{
    uint8_t *d = dst;
    uint8_t b = (uint8_t)c;
    if (((uintptr_t)d & 1) == 0 && n >= 4) {
        uint32_t v = b | ((uint32_t)b << 8);
        v |= v << 16;
        uint32_t *dl = (uint32_t *)d;
        for (; n >= 4; n -= 4)
            *dl++ = v;
        d = (uint8_t *)dl;
    }
    while (n--)
        *d++ = b;
    return dst;
}

NO_LIBCALL int memcmp(const void *a, const void *b, size_t n)
{
    const uint8_t *x = a, *y = b;
    for (; n; n--, x++, y++)
        if (*x != *y)
            return *x < *y ? -1 : 1;
    return 0;
}

NO_LIBCALL size_t strlen(const char *s)
{
    const char *p = s;
    while (*p)
        p++;
    return (size_t)(p - s);
}

NO_LIBCALL int strcmp(const char *a, const char *b)
{
    for (; *a && *a == *b; a++, b++)
        ;
    return (unsigned char)*a < (unsigned char)*b ? -1 : (unsigned char)*a > (unsigned char)*b;
}

NO_LIBCALL char *strcpy(char *restrict dst, const char *restrict src)
{
    char *d = dst;
    while ((*d++ = *src++) != 0)
        ;
    return dst;
}

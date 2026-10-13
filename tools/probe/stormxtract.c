/* Ground-truth MPQ extractor: uses the shipped Storm.dll to extract a file so
 * the Rust reader can be diffed against the real implementation.
 *
 * Storm.dll exports by ordinal only. Ordinals used here were cross-checked
 * against REA addresses: #266 -> RVA 0x1D3B0 (SFileOpenArchive) and
 * #268 -> RVA 0x19B60 (SFileOpenFileEx, ev_e527eb6922a83572be365705b25878a5bb65ad5a9cf88d76bdfdddcd8513c25a).
 *
 * build: i686-w64-mingw32-gcc -O2 -o stormxtract.exe stormxtract.c
 * run:   wine stormxtract.exe <archive> <path> <out.bin>
 */
#include <windows.h>
#include <stdio.h>

typedef BOOL(WINAPI *fnOpenArchive)(LPCSTR, DWORD, DWORD, HANDLE *);
typedef BOOL(WINAPI *fnCloseArchive)(HANDLE);
typedef BOOL(WINAPI *fnOpenFileEx)(HANDLE, LPCSTR, DWORD, HANDLE *);
typedef BOOL(WINAPI *fnCloseFile)(HANDLE);
typedef DWORD(WINAPI *fnGetFileSize)(HANDLE, LPDWORD);
typedef BOOL(WINAPI *fnReadFile)(HANDLE, LPVOID, DWORD, LPDWORD, LPOVERLAPPED);

#define ORD_OPENARCHIVE    266
#define ORD_CLOSEARCHIVE   252
#define ORD_OPENFILEEX     268
#define ORD_CLOSEFILE      253
#define ORD_GETFILESIZE    265
#define ORD_READFILE       269

#define SFILE_OPEN_FROM_MPQ 0x00000000
#define SFILE_OPEN_EXISTING 0x00000000

static void *ord(void *mod, WORD o) { return (void *)GetProcAddress(mod, MAKEINTRESOURCEA(o)); }

int main(int argc, char **argv) {
    if (argc < 4) {
        fprintf(stderr, "usage: stormxtract <archive> <path> <out.bin>\n");
        return 2;
    }
    HMODULE m = LoadLibraryA("Storm.dll");
    if (!m) { fprintf(stderr, "NO_STORM\n"); return 3; }

    fnOpenArchive  oa = (fnOpenArchive)ord(m, ORD_OPENARCHIVE);
    fnCloseArchive ca = (fnCloseArchive)ord(m, ORD_CLOSEARCHIVE);
    fnOpenFileEx   of = (fnOpenFileEx)ord(m, ORD_OPENFILEEX);
    fnCloseFile    cf = (fnCloseFile)ord(m, ORD_CLOSEFILE);
    fnGetFileSize  gs = (fnGetFileSize)ord(m, ORD_GETFILESIZE);
    fnReadFile     rf = (fnReadFile)ord(m, ORD_READFILE);
    if (!oa || !ca || !of || !cf || !gs || !rf) { fprintf(stderr, "NO_EXPORTS\n"); return 3; }

    HANDLE mpq = NULL;
    if (!oa(argv[1], 0, 0, &mpq) || !mpq || mpq == INVALID_HANDLE_VALUE) {
        fprintf(stderr, "OPEN_ARCHIVE_FAILED %lu\n", (unsigned long)GetLastError());
        return 4;
    }

    HANDLE f = NULL;
    if (!of(mpq, argv[2], SFILE_OPEN_FROM_MPQ, &f) || !f) {
        fprintf(stderr, "OPENFILE_FAILED %lu\n", (unsigned long)GetLastError());
        ca(mpq);
        return 6;
    }

    DWORD hi = 0;
    DWORD size = gs(f, &hi);
    if (size == 0xFFFFFFFFu) { fprintf(stderr, "GETFILESIZE_FAILED\n"); cf(f); ca(mpq); return 7; }

    unsigned char *buf = (unsigned char *)HeapAlloc(GetProcessHeap(), 0, size ? size : 1);
    DWORD got = 0;
    BOOL r = rf(f, buf, size, &got, NULL);
    if (!r) { fprintf(stderr, "READ_FAILED\n"); cf(f); ca(mpq); return 8; }

    FILE *out = fopen(argv[3], "wb");
    if (!out) { fprintf(stderr, "OUT_OPEN_FAILED\n"); cf(f); ca(mpq); return 9; }
    fwrite(buf, 1, got, out);
    fclose(out);

    printf("OK size=%u read=%u\n", size, got);

    HeapFree(GetProcessHeap(), 0, buf);
    cf(f);
    ca(mpq);
    return 0;
}

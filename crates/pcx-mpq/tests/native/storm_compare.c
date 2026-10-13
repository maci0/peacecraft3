/* Local-only byte oracle. Build with i686-w64-mingw32-gcc; run under Wine next
 * to the user's Storm.dll, linked with storm.def's import library. PXC1 records come from export_storm_comparison.
 * Archive APIs use measured ordinals 266/268/265/269/252/253. #552 -> RVA e090
 * -> measured dispatcher dd30. Original bytes and output streams stay local. */
#include <windows.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef BOOL(WINAPI *OpenArchive)(LPCSTR,DWORD,DWORD,HANDLE*);
typedef BOOL(WINAPI *StormOpenFile)(HANDLE,LPCSTR,DWORD,HANDLE*);
typedef BOOL(WINAPI *CloseHandleStorm)(HANDLE);
typedef DWORD(WINAPI *GetSize)(HANDLE,LPDWORD);
typedef BOOL(WINAPI *ReadStorm)(HANDLE,LPVOID,DWORD,LPDWORD,LPOVERLAPPED);
typedef BOOL(WINAPI *Decode)(void*,int*,const void*,int);
#define LOAD(name,ordinal) do { FARPROC entry=GetProcAddress(module,MAKEINTRESOURCEA(ordinal)); memcpy(&name,&entry,sizeof(name)); if(!name)return 3; } while(0)
static int word(FILE *input,uint32_t *value){return fread(value,4,1,input)==1;}
static unsigned char *read_bytes(FILE *input,uint32_t size,unsigned padding){
    if(size>0x7fffffffU-padding)return NULL;
    unsigned char *bytes=(unsigned char*)calloc((size_t)size+padding+1,1);
    if(!bytes||fread(bytes,1,size,input)!=size){free(bytes);return NULL;}return bytes;
}
__declspec(dllimport) BOOL WINAPI pcxStormOpenArchive(LPCSTR,DWORD,DWORD,HANDLE*);
int main(int argc,char **argv){
    if(argc!=3){fprintf(stderr,"usage: storm_compare <archive> <PXC1-stream>\n");return 2;}
    /* Dynamic LoadLibrary triggers this DLL's alternate cipher seed (17bf0).
     * Keep a real PE import so process attachment uses the shipped static path. */
    volatile OpenArchive static_import=pcxStormOpenArchive;(void)static_import;
    HMODULE module=GetModuleHandleA("Storm.dll");if(!module)return 3;
    OpenArchive oa=NULL;StormOpenFile of=NULL;CloseHandleStorm ca=NULL,cf=NULL;
    GetSize gs=NULL;ReadStorm rf=NULL;Decode decode=NULL;
    LOAD(oa,266);LOAD(of,268);LOAD(ca,252);LOAD(cf,253);LOAD(gs,265);LOAD(rf,269);LOAD(decode,552);
    HANDLE archive=NULL;if(!oa(argv[1],0,0,&archive)){fprintf(stderr,"OPEN_ARCHIVE_FAILED %lu\n",GetLastError());return 4;}
    FILE *input=fopen(argv[2],"rb");char magic[4];if(!input||fread(magic,1,4,input)!=4||memcmp(magic,"PXC1",4))return 5;
    unsigned files=0,identical=0,mismatched=0,refused_files=0,codec_refused=0,kind_refused=0;
    unsigned opens=0,open_identical=0,open_mismatched=0,open_refused=0,read_refused=0;
    uint32_t name_size;
    for(;;){
        size_t name_header=fread(&name_size,1,4,input);
        if(name_header==0&&feof(input))break;
        if(name_header!=4)return 6;
        unsigned char *name=read_bytes(input,name_size,0);uint32_t size,count;
        if(!name||!word(input,&size)||!word(input,&count))return 6;
        unsigned char *expected=read_bytes(input,size,0);if(!expected)return 6;
        unsigned at=0;int match=1,refused=0;
        for(uint32_t i=0;i<count;i++){
            uint32_t kind,length,want;if(!word(input,&kind)||!word(input,&length)||!word(input,&want)||want>size-at)return 6;
            unsigned char *packed=read_bytes(input,length,32),*plain=(unsigned char*)calloc((size_t)want+1,1);
            if(!packed||!plain||want>0x7fffffffU)return 6;
            int produced=(int)want;
            if(kind==0){if(length!=want){match=0;}else memcpy(plain,packed,want);}
            else if(kind==1){if(!decode(plain,&produced,packed,(int)length)){codec_refused++;refused=1;}}
            else {kind_refused++;refused=1;}
            if(!refused&&(produced!=(int)want||memcmp(expected+at,plain,want)))match=0;
            at+=want;free(packed);free(plain);
        }
        files++;if(refused){refused_files++;}else{if(match&&at==size)identical++;else mismatched++;}
        HANDLE file=NULL;SetLastError(0);
        if(!of(archive,(const char*)name,0,&file)||!file){open_refused++;}
        else {
            opens++;DWORD high=0,native_size=gs(file,&high),got=0;
            if(high||native_size==0xffffffffU||native_size>0x7fffffffU){read_refused++;}
            else {
                unsigned char *native=(unsigned char*)malloc((size_t)native_size+1);
                if(!native||!rf(file,native,native_size,&got,NULL)){read_refused++;}
                else if(got==size&&native_size==size&&!memcmp(native,expected,size)){open_identical++;}
                else {open_mismatched++;}
                free(native);
            }
            cf(file);
        }
        free(name);free(expected);
    }
    printf("payload_files=%u identical=%u mismatched=%u refused_files=%u refused_codec_sectors=%u refused_kind_sectors=%u\n",files,identical,mismatched,refused_files,codec_refused,kind_refused);
    printf("archive_files_opened=%u identical=%u mismatched=%u refused_open=%u refused_read=%u\n",opens,open_identical,open_mismatched,open_refused,read_refused);
    fclose(input);ca(archive);return mismatched||open_mismatched||codec_refused||kind_refused?1:0;
}

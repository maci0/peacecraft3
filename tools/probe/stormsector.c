/* Decode exported audio sectors using the shipped Storm.dll, without its
 * archive lookup. Export #552 (RVA 0xE090) forwards four stdcall arguments
 * to the measured SComp dispatcher at RVA 0xDD30. */
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
typedef BOOL (WINAPI *Decompress)(void *, int *, const void *, int);
int main(int argc,char **argv) {
    if(argc!=3){fprintf(stderr,"usage: stormsector <sector-directory> <output.wav>\n");return 2;}
    HMODULE library=LoadLibraryA("Storm.dll");
    Decompress decode=library?(Decompress)GetProcAddress(library,MAKEINTRESOURCEA(552)):NULL;
    if(!decode){fprintf(stderr,"Storm decompressor unavailable\n");return 3;}
    char path[4096];snprintf(path,sizeof(path),"%s\\manifest.txt",argv[1]);
    FILE *manifest=fopen(path,"rb"),*out=fopen(argv[2],"wb");
    unsigned size,count,written=0;
    if(!manifest||!out||fscanf(manifest,"%u %u",&size,&count)!=2){fprintf(stderr,"invalid manifest/output\n");return 4;}
    for(unsigned i=0;i<count;i++){
        unsigned index,want;
        if(fscanf(manifest,"%u %u",&index,&want)!=2||index!=i||want>65536||want>size-written)return 5;
        snprintf(path,sizeof(path),"%s\\%u.bin",argv[1],i);
        FILE *input=fopen(path,"rb");if(!input)return 6;
        fseek(input,0,SEEK_END);long length=ftell(input);rewind(input);
        if(length<=0||length>65536)return 7;
        unsigned char packed[65536],plain[65536];
        if(fread(packed,1,length,input)!=(size_t)length)return 8;
        fclose(input);
        int produced=(int)want;
        if((unsigned)length==want){memcpy(plain,packed,want);}
        else if(!decode(plain,&produced,packed,(int)length)){fprintf(stderr,"sector %u mask %02x failed\n",i,packed[0]);return 9;}
        if(produced!=(int)want){fprintf(stderr,"sector %u length %d != %u\n",i,produced,want);return 10;}
        if(fwrite(plain,1,want,out)!=want)return 11;
        written+=want;
    }
    fclose(manifest);if(fclose(out)!=0||written!=size)return 12;
    printf("OK %u bytes from %u original Storm-decoded sectors\n",written,count);
    FreeLibrary(library);return 0;
}

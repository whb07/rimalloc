#define _GNU_SOURCE
#include <dlfcn.h>
#include <malloc.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

__attribute__((constructor)) static void check(void) {
    const char* expected = getenv("NGINX_BENCH_ALLOCATOR");
    if (!expected) return;
    const char* names[] = {"malloc", "calloc", "realloc", "free", "posix_memalign", "malloc_usable_size"};
    for (size_t i=0; i<sizeof(names)/sizeof(names[0]); ++i) {
        Dl_info info;
        char actual[4096], wanted[4096];
        void* symbol=dlsym(RTLD_DEFAULT,names[i]);
        if (!symbol || !dladdr(symbol,&info) || !realpath(info.dli_fname,actual)) _exit(121);
        bool valid = strcmp(expected,"glibc") == 0 ? strstr(actual,"/libc.so") != NULL :
                     realpath(expected,wanted) && strcmp(actual,wanted) == 0;
        if (!valid) { fprintf(stderr,"Nginx allocator guard: wrong %s owner: %s\n",names[i],actual); _exit(121); }
    }
    void* p=malloc(64); if (!p) _exit(121);
    memset(p,0x5a,64); p=realloc(p,128); if (!p || ((unsigned char*)p)[0]!=0x5a) _exit(121);
    bool (*owned)(const void*) = (bool(*)(const void*))dlsym(RTLD_DEFAULT,"mi_check_owned");
    if (owned && !owned(p)) _exit(121);
    free(p); p=NULL;
    if(posix_memalign(&p,64,128) || !p || ((size_t)p % 64) || malloc_usable_size(p)<128) _exit(121);
    free(p);
}

#include <stdlib.h>

void* plugin_alloc(size_t size) { return malloc(size); }
void plugin_free(void* p) { free(p); }

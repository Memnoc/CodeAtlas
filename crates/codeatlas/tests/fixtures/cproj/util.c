#include <string.h>

#include "util.h"

struct point {
    int x;
    int y;
};

static char *decorate(const char *name) {
    static char buf[64];
    strcpy(buf, "* ");
    strcat(buf, name);
    return buf;
}

char *util_greet(const char *name) {
    return decorate(name);
}

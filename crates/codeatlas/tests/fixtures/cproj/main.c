#include <stdio.h>

#include "app/app.h"
#include "util.h"

static void local_note(void) {
    puts("note");
}

int main(void) {
    app_run();
    printf("%s\n", util_greet("world"));
    local_note();
    return 0;
}

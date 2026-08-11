#include <stdio.h>

#include "app/app.h"
/* Neither beside this file nor at the repo root — app/config.h is the decoy. */
#include "config.h"
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

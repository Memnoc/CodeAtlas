#include "util.h"

/* The decoy for the value-receiver row. `o.util_greet(…)` reaches a function
   *pointer field* of a value, while `util.h` — included right here — really
   does declare `util_greet`, implemented in util.c. A resolver that read the
   field name as a plain callee would wire the two together. */
struct ops {
    char *(*util_greet)(const char *);
};

static char *run_through(struct ops o) {
    return o.util_greet("x");
}

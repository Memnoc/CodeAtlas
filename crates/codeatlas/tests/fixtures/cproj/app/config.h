#ifndef CONFIG_H
#define CONFIG_H

/* The decoy for the resolves-nowhere row. main.c includes "config.h", which
   is neither beside it nor at the repo root; this header has that name one
   directory down, so a resolver searching by name rather than by path would
   wire the two together. */
int config_value(void);

#endif

/* The decoy for the outside-the-repository row: a real, exported, repo-local
   definition named after the libc function main.c calls through <stdio.h>.
   Nothing includes this file, so the only way an edge to it can appear is a
   resolver matching callees by name across the tree. */
int printf(const char *fmt) {
    return fmt == 0;
}

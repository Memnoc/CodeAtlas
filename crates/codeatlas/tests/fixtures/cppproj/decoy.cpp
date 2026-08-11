// The decoy for the outside-the-repository row: a real, exported, repo-local
// definition named after the standard-library function main.cpp calls through
// <cstdio>. Nothing includes this file.
int puts(const char *s) {
    return s == 0;
}

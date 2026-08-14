#include "bare.hpp"

// The enclosing-namespace walk and the caller-scoped suppression (ticket
// 10's crosscheck repair). A bare callee written inside alg:: resolves by
// walking the caller's own namespace path outward within this file; the
// global-scope caller at the bottom carries no path, so its bare nsq(4)
// legitimately reaches the global nsq that bare.hpp fronts — the edge a
// file-wide tail-suppression wrongly swallowed.
namespace alg {
int nsq(int x) {
    return x * x;
}

int dbl(int x) {
    return x + x;
}

namespace inner {
int nsq(int x) {
    return -(x * x);
}

// Both alg::inner::nsq and alg::nsq exist; C++ lookup walks outward from
// the innermost enclosing namespace, so the inner one answers.
int f(int k) {
    return nsq(k);
}

// Only alg::dbl exists; the walk skips inner and lands one level out.
int g(int k) {
    return dbl(k);
}
}
}

// Global scope: unqualified lookup here cannot see alg::nsq, so this call
// belongs to the global nsq declared in bare.hpp and implemented in bare.cpp.
int use_global(int k) {
    return nsq(k);
}

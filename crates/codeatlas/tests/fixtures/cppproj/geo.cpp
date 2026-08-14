#include "bare.hpp"
#include "geo.hpp"

namespace geo {
int nsq(int x) {
    return x * x;
}

// An unqualified call to a sibling in the same namespace. Resolving it needs
// the scope tracking ticket 10 excludes, so it stays unresolved this lap —
// and it must not be offered to the includes either: bare.hpp fronts a
// *global* nsq, and C++ name lookup gives this call to geo::nsq above.
int twice(int k) {
    return nsq(k) + nsq(k);
}
}

// The compact C++17 spelling; the classic nested blocks are in geo.hpp.
namespace geo::inner {
int deep(int x) {
    // A qualified call to the enclosing namespace, resolved in this file.
    return geo::nsq(x) + 1;
}
}

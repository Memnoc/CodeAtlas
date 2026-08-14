#ifndef GEO_HPP
#define GEO_HPP

// Namespaced declarations. The prototypes promise geo.cpp implements the
// names, and both are stored and exported fully qualified — geo::nsq is the
// form every call site writes (ticket 10).
namespace geo {
int nsq(int x);
struct Disc {
    double r;
};
namespace inner {
int deep(int x);
}
}

#endif

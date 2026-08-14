#include "geo.hpp"

// Qualified calls as idiomatic C++ writes them. The qualified name is the
// stored name, so both resolve through the header to the implementations in
// geo.cpp — no receiver-module binding involved.
int use_geo() {
    return geo::nsq(2);
}

int use_deep() {
    return geo::inner::deep(3);
}

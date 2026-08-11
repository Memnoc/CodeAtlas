#include <cstdio>
#include <iostream>

#include "geometry.hpp"
#include "legacy.h"
#include "report.hpp"
// Neither beside this file nor at the repo root — detail/shapes.hpp is the
// decoy.
#include "shapes.hpp"

int main() {
    legacy_go();
    Circle c(2.0);
    report(tau());
    // A member call on a value. `area` is also a free function this file can
    // reach through geometry.hpp, so nothing may connect these two.
    std::cout << c.area() << "\n";
    // A call into the standard library, which is outside the repository —
    // and decoy.cpp defines a `puts` of its own.
    puts("done");
    return 0;
}

#include <iostream>

#include "geometry.hpp"
#include "legacy.h"
#include "report.hpp"

int main() {
    legacy_go();
    Circle c(2.0);
    report(tau());
    std::cout << c.area() << "\n";
    return 0;
}

#include "geometry.hpp"

static double square(double x) {
    return x * x;
}

double tau() {
    return 6.28318530717958647692;
}

Circle::Circle(double r) : r_(r) {}

double Circle::area() const {
    return tau() * square(r_) / 2.0;
}

// The free function the header promises: same name as the method above, and
// reachable from main.cpp through `#include "geometry.hpp"`.
double area() {
    return 1.0;
}

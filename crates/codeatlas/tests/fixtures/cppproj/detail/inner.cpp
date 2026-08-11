// A parent-relative include. This is the only place in the fixture where
// `../` traversal is what makes the edge, so the row cannot be satisfied by a
// same-directory include standing in for it.
#include "../geometry.hpp"

double twice_tau() {
    return 2.0 * tau();
}

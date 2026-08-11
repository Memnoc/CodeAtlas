#ifndef DETAIL_SHAPES_HPP
#define DETAIL_SHAPES_HPP

// The decoy for the resolves-nowhere row. main.cpp includes "shapes.hpp",
// which is neither beside it nor at the repo root; this header has that name
// one directory down, so a resolver searching by name rather than by path
// would wire the two together.
double unit_area();

#endif

#ifndef BARE_HPP
#define BARE_HPP

// The decoy for the unqualified-sibling suppression: a global nsq, declared
// in a header geo.cpp includes and implemented in this header's pair. The
// unqualified nsq(k) inside namespace geo belongs to geo::nsq by C++ name
// lookup; a resolver that offered the bare name to the includes would land
// here instead.
int nsq(int x);

#endif

#ifndef GEOMETRY_HPP
#define GEOMETRY_HPP

class Circle {
  public:
    explicit Circle(double r);
    double area() const;
    double radius() const { return r_; }

  private:
    double r_;
};

double tau();

// The decoy for the value-receiver row: a free function sharing its name with
// `Circle::area`, declared in a header main.cpp includes and implemented in
// that header's pair. `c.area()` in main.cpp calls the *method* on a value; a
// resolver that read the member name as a plain callee would land here.
double area();

#endif

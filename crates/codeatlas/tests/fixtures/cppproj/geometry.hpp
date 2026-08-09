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

#endif

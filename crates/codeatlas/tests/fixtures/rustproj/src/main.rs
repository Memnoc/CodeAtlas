mod shapes;
mod util;

use crate::shapes::Circle;
use crate::util::greet;

fn main() {
    let c = Circle { radius: 2.0 };
    let message = greet("atlas");
    println!("{} {}", message, c.area());
}

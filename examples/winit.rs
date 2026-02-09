// MIT/Apache2/ZLib License

use winit::event_loop::EventLoop;

mod util;

fn main() {
    util::run(EventLoop::new().unwrap());
}

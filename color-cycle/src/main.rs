use gr_context::Context;
use opengles::glesv2 as gl;
use std::thread;
use std::time::{Duration, Instant};

const STEPS: u32 = 180;
const MILLIS_PER_FRAME: Duration = Duration::from_millis((1000_f64 / 60_f64) as u64);

pub fn draw(context: &mut Context, progress: f32) {
  gl::clear_color(1.0_f32 - progress, progress, 0.0, 1.0);
  gl::clear(gl::GL_COLOR_BUFFER_BIT);
  context.swap_buffers();
}

fn main() {
  let mut context = Context::new();

  for i in 0..STEPS {
    let start = Instant::now();
    draw(&mut context, i as f32 / STEPS as f32);

    let end = Instant::now();

    match start
      .checked_add(MILLIS_PER_FRAME)
      .expect("Can always add 16ms")
      .checked_duration_since(end)
    {
      Some(sleep) => thread::sleep(sleep),
      None => {}
    };
  }
}

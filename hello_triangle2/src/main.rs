#![allow(non_upper_case_globals)]

use const_format::formatcp;
use gr_context::Context;
use opengles::glesv2 as gl;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;

fn gl_check() {
  let err = gl::get_error();
  if err == 0 {
    return;
  }

  println!("glGetError is non zero: {:04x}", err);

  match err {
    gl::GL_INVALID_OPERATION => println!("Given when the set of state for a command is \
    not legal for the parameters given to that command. It is also given for commands where combinations of parameters define what the legal parameters are."),
    _ => (),
  };

  println!("\nCheck https://www.khronos.org/opengl/wiki/OpenGL_Error");
  panic!();
}

// --------------------------------------------------------------------------------

#[rustfmt::skip]
static VERTEX_DATA: [gl::GLfloat; 16] = [
  -1.0, -1.0,  1.0,  1.0,
   1.0, -1.0,  1.0,  1.0,
   1.0,  1.0,  1.0,  1.0,
  -1.0,  1.0,  1.0,  1.0,
];

const VSHADER_SOURCE: &str = "
attribute mediump vec4 vertex;
varying mediump vec2 tcoord;

void main(void) {
  mediump vec4 pos = vertex;
  gl_Position = pos;
  tcoord = vertex.xy * 0.5 + 0.5;
}
";

/*
 * VC4 (Raspberry Pi up to 3) maxes out at 18 iterations. Higher values will cause an
 * OOM (GL_OUT_OF_MEMORY).
 */

#[cfg(feature = "vc4")]
const MANDELBROT_MAX_ITERATIONS: i32 = 18;
#[cfg(feature = "vc4")]
const MANDELBROT_FRAG_COLOR_EXPR: &str = formatcp!(
  "vec4(float(i) * (1.0 / {}.0), 0, 0, 1)",
  MANDELBROT_MAX_ITERATIONS
);

/*
 * VC6 (Raspberry Pi 4+) can do thousands of iterations, the demo will just take longer
 * to do the initial render.
 */

#[cfg(feature = "vc6")]
const MANDELBROT_MAX_ITERATIONS: i32 = 512;
#[cfg(feature = "vc6")]
const MANDELBROT_FRAG_COLOR_EXPR: &str =
  "float(i > 0) * hsl2rgb(vec3(float(i) / 360.0, 1.0, 0.5), 1.0)";

// Mandelbrot
const MANDELBROT_FSHADER_SOURCE: &str = formatcp!(
  "
uniform mediump vec4 color;
uniform mediump vec2 scale;
uniform mediump vec2 centre;
varying mediump vec2 tcoord;

mediump vec4 hsl2rgb(in mediump vec3 c, in mediump float a) {{
  mediump vec3 rgb = clamp(
    abs(mod(c.x * 6.0 + vec3(0.0, 4.0, 2.0), 6.0) - 3.0) - 1.0, 0.0, 1.0
  );

  return vec4(c.z + c.y * (rgb - 0.5) * (1.0 - abs(2.0 * c.z - 1.0)), a);
}}

void main(void) {{
  mediump float intensity;
  mediump vec4 color2;
  mediump float cr = (gl_FragCoord.x - centre.x) * scale.x;
  mediump float ci = (gl_FragCoord.y - centre.y) * scale.y;
  mediump float ar = cr;
  mediump float ai = ci;
  mediump float tr, ti;
  mediump float col = 0.0;
  mediump float p = 0.0;
  mediump int i = 0;

  for (mediump int i2 = 1; i2 < {}; i2++) {{
    tr = ar * ar - ai * ai + cr;
    ti = 2.0 * ar * ai + ci;
    p = tr * tr + ti * ti;
    ar = tr;
    ai = ti;
    if (p > 16.0) {{
      i = i2;
      break;
    }}
  }}

  gl_FragColor = {};
}}
",
  MANDELBROT_MAX_ITERATIONS,
  MANDELBROT_FRAG_COLOR_EXPR,
);

// Julia
const JULIA_FSHADER_SOURCE: &str = "
uniform mediump vec4 color;
uniform mediump vec2 scale;
uniform mediump vec2 centre;
uniform mediump vec2 offset;
varying mediump vec2 tcoord;
uniform sampler2D tex;

void main(void) {
  mediump float intensity;
  mediump vec4 color2;
  mediump float ar = (gl_FragCoord.x - centre.x) * scale.x;
  mediump float ai = (gl_FragCoord.y - centre.y) * scale.y;
  mediump float cr = (offset.x - centre.x) * scale.x;
  mediump float ci = (offset.y - centre.y) * scale.y;
  mediump float tr,ti;
  mediump float col = 0.0;
  mediump float p = 0.0;
  lowp int i = 0;
  mediump vec2 t2;
  t2.x = tcoord.x + (offset.x - centre.x) * (0.5/centre.y);
  t2.y = tcoord.y + (offset.y - centre.y) * (0.5/centre.x);

  for(int i2 = 1; i2 < 16; i2++) {
    tr = ar * ar - ai * ai + cr;
    ti = 2.0 * ar * ai + ci;
    p = tr * tr + ti * ti;
    ar = tr;
    ai = ti;
    if (p > 16.0) {
      i = i2;
      break;
    }
  }
  color2 = vec4(0, float(i) * 0.0625, 0, 1);
  color2 = color2 + texture2D(tex, t2);
  gl_FragColor = color2;
}
";

// --------------------------------------------------------------------------------

fn print_shader_info_log(shader: gl::GLuint) {
  // Prints the compile log for a shader
  match gl::get_shader_info_log(shader, 1024) {
    Some(log) => println!("{}:shader:\n{}\n", shader, log),
    _ => {}
  }
}

fn print_program_info_log(program: gl::GLuint) {
  // Prints the information log for a program object
  match gl::get_program_info_log(program, 1024) {
    Some(log) => println!("{}:program:\n{}\n", program, log),
    _ => {}
  }
}

// --------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct CubeState {
  screen_width: u32,
  screen_height: u32,

  dispman_display: u32,
  dispman_update: u32,
  dispman_element: u32,

  verbose: gl::GLuint,
  vshader: gl::GLuint,
  fshader: gl::GLuint,
  mshader: gl::GLuint,
  program: gl::GLuint,
  program2: gl::GLuint,
  tex_fb: gl::GLuint,
  tex: gl::GLuint,
  buf: gl::GLuint,

  // julia attribs
  unif_color: gl::GLint,
  attr_vertex: gl::GLuint,
  unif_scale: gl::GLint,
  unif_offset: gl::GLint,
  unif_tex: gl::GLint,
  unif_centre: gl::GLint,

  // mandelbrot attribs
  attr_vertex2: gl::GLuint,
  unif_scale2: gl::GLint,
  unif_offset2: gl::GLint,
  unif_centre2: gl::GLint,
}

impl CubeState {
  pub fn new() -> Self {
    return CubeState {
      screen_width: 0,
      screen_height: 0,

      dispman_display: 0,
      dispman_update: 0,
      dispman_element: 0,

      verbose: 1,
      vshader: 0,
      fshader: 0,
      mshader: 0,
      program: 0,
      program2: 0,
      tex_fb: 0,
      tex: 0,
      buf: 0,

      // julia attribs
      unif_color: 0,
      attr_vertex: 0,
      unif_scale: 0,
      unif_offset: 0,
      unif_tex: 0,
      unif_centre: 0,

      // mandelbrot attribs
      attr_vertex2: 0,
      unif_scale2: 0,
      unif_offset2: 0,
      unif_centre2: 0,
    };
  }
}

/***********************************************************
 * Name: init_ogl
 *
 * Arguments:
 *       CUBE_STATE_T *state - holds OGLES model info
 *
 * Description: Sets the display, OpenGL|ES context and screen stuff
 *
 * Returns: void
 *
 ***********************************************************/
pub fn init_ogl(context: &mut Context, state: &mut CubeState) {
  // lmfr: This only runs if selecting "G1 Legacy - Original non-GL desktop driver"
  // lmfr: in raspi-config

  state.screen_width = context.width();
  state.screen_height = context.height();

  // Set background color and clear buffers
  gl::clear_color(0.15f32, 0.25f32, 0.35f32, 1.0f32);
  gl::clear(gl::GL_COLOR_BUFFER_BIT);

  gl_check();
}

pub fn init_shaders(state: &mut CubeState) {
  state.vshader = gl::create_shader(gl::GL_VERTEX_SHADER);
  gl::shader_source(state.vshader, VSHADER_SOURCE.as_bytes());
  gl::compile_shader(state.vshader);
  gl_check();

  if state.verbose != 0 {
    print_shader_info_log(state.vshader);
  }

  state.fshader = gl::create_shader(gl::GL_FRAGMENT_SHADER);
  gl::shader_source(state.fshader, JULIA_FSHADER_SOURCE.as_bytes());
  gl::compile_shader(state.fshader);
  gl_check();

  if state.verbose != 0 {
    print_shader_info_log(state.fshader);
  }

  state.mshader = gl::create_shader(gl::GL_FRAGMENT_SHADER);
  gl::shader_source(state.mshader, MANDELBROT_FSHADER_SOURCE.as_bytes());
  gl::compile_shader(state.mshader);
  gl_check();

  if state.verbose != 0 {
    print_shader_info_log(state.mshader);
  }

  // julia
  state.program = gl::create_program();
  gl::attach_shader(state.program, state.vshader);
  gl::attach_shader(state.program, state.fshader);
  gl::link_program(state.program);
  gl_check();

  if state.verbose != 0 {
    print_program_info_log(state.program);
  }

  state.attr_vertex = gl::get_attrib_location(state.program, "vertex") as gl::GLuint;
  gl_check();
  state.unif_color = gl::get_uniform_location(state.program, "color");
  gl_check();
  state.unif_scale = gl::get_uniform_location(state.program, "scale");
  gl_check();
  state.unif_offset = gl::get_uniform_location(state.program, "offset");
  gl_check();
  state.unif_tex = gl::get_uniform_location(state.program, "tex");
  gl_check();
  state.unif_centre = gl::get_uniform_location(state.program, "centre");
  gl_check();

  // mandelbrot
  state.program2 = gl::create_program();
  gl_check();
  gl::attach_shader(state.program2, state.vshader);
  gl_check();
  gl::attach_shader(state.program2, state.mshader);
  gl_check();
  gl::link_program(state.program2);
  gl_check();

  state.attr_vertex2 = gl::get_attrib_location(state.program2, "vertex") as gl::GLuint;
  state.unif_scale2 = gl::get_uniform_location(state.program2, "scale");
  state.unif_offset2 = gl::get_uniform_location(state.program2, "offset");
  state.unif_centre2 = gl::get_uniform_location(state.program2, "centre");
  gl_check();

  gl::clear_color(0.0, 1.0, 1.0, 1.0);

  state.buf = gl::gen_buffers(1)[0];

  gl_check();

  // Prepare a texture image
  state.tex = gl::gen_textures(1)[0];
  gl_check();
  gl::bind_texture(gl::GL_TEXTURE_2D, state.tex);
  gl_check();
  gl::tex_image_2d(
    gl::GL_TEXTURE_2D,                  /* target */
    0,                                  /* level */
    gl::GL_RGB as i32,                  /* internal_format */
    state.screen_width as gl::GLsizei,  /* width */
    state.screen_height as gl::GLsizei, /* height */
    0,                                  /* border */
    gl::GL_RGB,                         /* src_format */
    gl::GL_UNSIGNED_SHORT_5_6_5,        /* src_type */
    &[] as &[gl::GLchar; 0],            /* buffer */
  );
  gl_check();

  gl::tex_parameterf(
    gl::GL_TEXTURE_2D,
    gl::GL_TEXTURE_MIN_FILTER,
    gl::GL_NEAREST as f32,
  );
  gl::tex_parameterf(
    gl::GL_TEXTURE_2D,
    gl::GL_TEXTURE_MAG_FILTER,
    gl::GL_NEAREST as f32,
  );
  gl_check();

  // Prepare a framebuffer for rendering
  state.tex_fb = gl::gen_framebuffers(1)[0];
  gl_check();
  gl::bind_framebuffer(gl::GL_FRAMEBUFFER, state.tex_fb);
  gl_check();
  gl::framebuffer_texture_2d(
    gl::GL_FRAMEBUFFER,
    gl::GL_COLOR_ATTACHMENT0,
    gl::GL_TEXTURE_2D,
    state.tex,
    0,
  );
  gl_check();
  gl::bind_framebuffer(gl::GL_FRAMEBUFFER, 0);
  gl_check();

  // Prepare viewport
  gl::viewport(0, 0, state.screen_width as i32, state.screen_height as i32);
  gl_check();

  // Upload vertex data to a buffer
  gl::bind_buffer(gl::GL_ARRAY_BUFFER, state.buf);
  gl::buffer_data(gl::GL_ARRAY_BUFFER, &VERTEX_DATA, gl::GL_STATIC_DRAW);
  gl::vertex_attrib_pointer_offset(
    state.attr_vertex, /* index */
    4,                 /* size */
    gl::GL_FLOAT,      /* type */
    false,             /* normalized */
    16,                /* stride */
    0,                 /* offset */
  );
  gl::enable_vertex_attrib_array(state.attr_vertex);
  gl::vertex_attrib_pointer_offset(
    state.attr_vertex2, /* index */
    4,                  /* size */
    gl::GL_FLOAT,       /* type */
    false,              /* normalized */
    16,                 /* stride */
    0,                  /* offset */
  );
  gl::enable_vertex_attrib_array(state.attr_vertex2);

  gl_check();
}

fn draw_mandelbrot_to_texture(
  state: &mut CubeState,
  cx: gl::GLfloat,
  cy: gl::GLfloat,
  scale: gl::GLfloat,
) {
  // Draw the mandelbrot to a texture
  gl::bind_framebuffer(gl::GL_FRAMEBUFFER, state.tex_fb);
  gl_check();
  gl::bind_buffer(gl::GL_ARRAY_BUFFER, state.buf);

  gl::use_program(state.program2);
  gl_check();

  gl::uniform2f(state.unif_scale2, scale, scale);
  gl::uniform2f(state.unif_centre2, cx, cy);
  gl_check();
  gl::draw_arrays(gl::GL_TRIANGLE_FAN, 0, 4);
  gl_check();

  gl::flush();
  gl::finish();
  gl_check();
}

fn draw_triangles(
  state: &mut CubeState,
  cx: gl::GLfloat,
  cy: gl::GLfloat,
  scale: gl::GLfloat,
  x: i32,
  y: i32,
) {
  // Now render to the main frame buffer
  gl::bind_framebuffer(gl::GL_FRAMEBUFFER, 0);
  // // Clear the background (not really necessary I suppose)
  gl::clear(gl::GL_COLOR_BUFFER_BIT | gl::GL_DEPTH_BUFFER_BIT);
  gl_check();

  gl::bind_buffer(gl::GL_ARRAY_BUFFER, state.buf);
  gl_check();
  gl::use_program(state.program);
  gl_check();
  gl::bind_texture(gl::GL_TEXTURE_2D, state.tex);
  gl_check();
  gl::uniform4f(state.unif_color, 0.5, 0.5, 0.8, 1.0);
  gl::uniform2f(state.unif_scale, scale, scale);
  gl::uniform2f(state.unif_offset, x as gl::GLfloat, y as gl::GLfloat);
  gl::uniform2f(state.unif_centre, cx, cy);
  gl::uniform1i(state.unif_tex, 0); // I don't really understand this part, perhaps it relates to active texture?
  gl_check();

  gl::draw_arrays(gl::GL_TRIANGLE_FAN, 0, 4);
  gl_check();

  gl::bind_buffer(gl::GL_ARRAY_BUFFER, 0);

  gl::flush();
  gl::finish();
  gl_check();
}

const X_SIGN: u8 = 1 << 4;
const Y_SIGN: u8 = 1 << 5;

fn get_mouse(state: &mut CubeState, mouse_dev: &mut File, outx: &mut i32, outy: &mut i32) -> bool {
  let width = state.screen_width;
  let height = state.screen_height;
  // let mut x: i32 = 800;
  // let mut y: i32 = 400;
  let mut x: i32 = *outx;
  let mut y: i32 = *outy;

  let mut buttons: u8;
  let mut dx: i8;
  let mut dy: i8;
  let mut buf: [u8; 3] = [0, 0, 0];
  loop {
    // let &mut mouse_dev = maybe_mouse_dev.as_ref().unwrap();
    match mouse_dev.read(&mut buf) {
      Ok(count) => {
        buttons = buf[0];
        dx = buf[1] as i8;
        dy = buf[2] as i8;
        if count < 3_usize {
          if *outx != 0i32 {
            *outx = x;
          }
          if *outy != 0i32 {
            *outy = y;
          }
          return false;
        }
        if buttons & 8 != 0u8 {
          break; // This bit should always be set
        }
      }
      _ => {}
    }
  }

  if buttons & 3 != 0u8 {
    return buttons & 3 != 0u8;
  }

  x += dx as i32;
  y += dy as i32;

  if buttons & X_SIGN != 0 {
    x -= 256;
  }
  if buttons & Y_SIGN != 0 {
    y -= 256;
  }
  if x < 0 {
    x = 0;
  }
  if y < 0 {
    y = 0;
  }
  if x as u32 > width {
    x = width as i32;
  }
  if y as u32 > height {
    y = height as i32;
  }

  *outx = x;
  *outy = y;

  return false;
}

fn demo(context: &mut Context, state: &mut CubeState) {
  let terminate: bool = false;

  // if (bcm_host::get_processor_id() == PROCESSOR_BCM2838) {
  //   panic!("This demo application is not available on the Pi4\n\n");
  // }

  // Start OGLES
  init_ogl(context, state);
  init_shaders(state);

  let cx: gl::GLfloat = state.screen_width as gl::GLfloat / 2 as gl::GLfloat;
  let cy: gl::GLfloat = state.screen_height as gl::GLfloat / 2 as gl::GLfloat;

  draw_mandelbrot_to_texture(state, cx, cy, 0.003);

  let mut mouse_dev: File;
  let mut maybe_mouse_dev: Option<&mut File> = None;
  let mut x: i32 = 800i32;
  let mut y: i32 = 400i32;

  while !terminate {
    match maybe_mouse_dev {
      None => {
        let new_mouse_dev = OpenOptions::new()
          .read(true)
          .custom_flags(libc::O_NONBLOCK)
          .open("/dev/input/mouse0");
        match new_mouse_dev {
          Ok(dev) => {
            mouse_dev = dev;
            maybe_mouse_dev = Some(&mut mouse_dev)
          }
          _ => {}
        }
      }
      Some(ref mut mouse_dev) => {
        let b: bool = get_mouse(state, mouse_dev, &mut x, &mut y);
        if b {
          break;
        }
      }
    }

    draw_triangles(state, cx, cy, 0.003, x, y);
    context.swap_buffers();
    gl_check();
  }
}

fn main() {
  let mut context = Context::new();

  let mut state: CubeState = CubeState::new();
  demo(&mut context, &mut state);
}

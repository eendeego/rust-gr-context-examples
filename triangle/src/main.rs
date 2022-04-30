use gr_context::Context;
use opengles::glesv2 as gl;
use std::f64::consts::PI;
use std::fs::File;
use std::io::prelude::*;
use std::mem::size_of;
use std::thread;
use std::time::Duration;

// ----------------------------------------------------------------------------

pub mod ffi {
  use super::*;

  extern "C" {
    pub fn glGetIntegerv(pname: gl::GLenum, params: *mut gl::GLint);
    // pub fn glGetAttribLocation(program: gl::GLuint, name: *const gl::GLchar) -> gl::GLint;
    // pub fn glGetUniformLocation(program: gl::GLuint, name: *const gl::GLchar) -> gl::GLint;
    pub fn glUniformMatrix4fv(
      location: gl::GLint,
      count: gl::GLsizei,
      transpose: gl::GLboolean,
      value: *const gl::GLfloat,
    );
  }
}

pub fn mygl_get_viewport(name: gl::GLenum) -> [gl::GLint; 4] {
  unsafe {
    let mut value: [gl::GLint; 4] = [0, 0, 0, 0];

    ffi::glGetIntegerv(name, &mut value as *mut gl::GLint);

    value
  }
}

// ----------------------------------------------------------------------------

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

fn gl_check() {
  let err = gl::get_error();
  if err == 0 {
    return;
  }

  println!("\nglGetError is non zero: {:04x}\n", err);
  println!("Check https://www.khronos.org/opengl/wiki/OpenGL_Error\n\n");
  panic!();
}

// ----------------------------------------------------------------------------

#[rustfmt::skip]
const VERTEX_COLOR: [gl::GLfloat; 12] = [
  1_f32 as gl::GLfloat, 0_f32 as gl::GLfloat, 0_f32 as gl::GLfloat, 1_f32 as gl::GLfloat,
  0_f32 as gl::GLfloat, 1_f32 as gl::GLfloat, 0_f32 as gl::GLfloat, 1_f32 as gl::GLfloat,
  0_f32 as gl::GLfloat, 0_f32 as gl::GLfloat, 1_f32 as gl::GLfloat, 1_f32 as gl::GLfloat,
];

const VERTEX_SHADER_SOURCE: &str = "
attribute mediump vec3 vertexPosition;
attribute vec4 vertexColor;

uniform mediump mat4 projectionMatrix;
uniform mediump mat4 modelViewMatrix;

varying mediump vec4 color;

void main() {
  gl_Position = projectionMatrix * modelViewMatrix * vec4(vertexPosition, 1.0);
  color = vertexColor;
}
";

const FRAGMENT_SHADER_SOURCE: &str = "
varying mediump vec4 color;

void main() {
  gl_FragColor = color;
}
";

fn init_shader(program: gl::GLuint, type_: gl::GLenum, source: &str) -> gl::GLuint {
  // Create a Vertex Shader
  let shader: gl::GLuint = gl::create_shader(type_);
  gl_check();

  // Set source for shader
  gl::shader_source(shader, source.as_bytes());
  gl_check();

  // Compile shader
  gl::compile_shader(shader);
  print_shader_info_log(shader);
  gl_check();

  gl::attach_shader(program, shader);
  gl_check();

  return shader;
}

pub struct Env {
  pub vertex_position_buffer: gl::GLuint,
  pub vertex_color_buffer: gl::GLuint,
  pub vertex_position: gl::GLuint,
  pub vertex_color: gl::GLuint,
  pub projection_matrix: gl::GLint,
  pub model_view_matrix: gl::GLint,
  pub vertices: [gl::GLfloat; 9],
}

#[rustfmt::skip]
#[inline(always)]
pub fn identity() -> [f32; 16] {
  [
    1 as gl::GLfloat, 0 as gl::GLfloat, 0 as gl::GLfloat, 0 as gl::GLfloat,
    0 as gl::GLfloat, 1 as gl::GLfloat, 0 as gl::GLfloat, 0 as gl::GLfloat,
    0 as gl::GLfloat, 0 as gl::GLfloat, 1 as gl::GLfloat, 0 as gl::GLfloat,
    0 as gl::GLfloat, 0 as gl::GLfloat, 0 as gl::GLfloat, 1 as gl::GLfloat,
  ]
}

#[rustfmt::skip]
#[inline]
pub fn orthographic(
  top: gl::GLfloat,
  right: gl::GLfloat,
  bottom: gl::GLfloat,
  left: gl::GLfloat,
  near: gl::GLfloat,
  far: gl::GLfloat,
) -> [gl::GLfloat; 16] {
  let w = right - left;
  let h = top - bottom;
  let p = far - near;

  let x = (right + left) / w;
  let y = (top + bottom) / h;
  let z = (far + near) / p;

  [
    (2_f32 / w) as gl::GLfloat, (    0_f32) as gl::GLfloat, (     0_f32) as gl::GLfloat, (0_f32) as gl::GLfloat,
    (    0_f32) as gl::GLfloat, (2_f32 / h) as gl::GLfloat, (     0_f32) as gl::GLfloat, (0_f32) as gl::GLfloat,
    (    0_f32) as gl::GLfloat, (    0_f32) as gl::GLfloat, (-2_f32 / p) as gl::GLfloat, (0_f32) as gl::GLfloat,
    (       -x) as gl::GLfloat, (       -y) as gl::GLfloat, (        -z) as gl::GLfloat, (1_f32) as gl::GLfloat,
  ]
}

fn matrices(width: u32, height: u32) -> ([gl::GLfloat; 16], [gl::GLfloat; 16]) {
  let ratio = (width as f32) / (height as f32);
  let scale = 3_f32;

  let left = -scale * ratio / 2_f32;
  let right = scale * ratio / 2_f32;
  let bottom = -scale / 2_f32;
  let top = scale / 2_f32;

  let near = -1.0_f32;
  let far = 1.0_f32;

  let projection = orthographic(top, right, bottom, left, near, far);
  let model_view = identity();

  return (projection, model_view);
}

fn compute_triangle(vertices: &mut [gl::GLfloat; 9]) {
  for i in 0..3 {
    vertices[i * 3 + 0] = (PI / 2.0 + (i as f64 + 1.0) * 2.0 * PI / 3.0).cos() as f32;
    vertices[i * 3 + 1] = (PI / 2.0 + (i as f64 + 1.0) * 2.0 * PI / 3.0).sin() as f32;
    vertices[i * 3 + 2] = 0f32;
  }
}

pub fn setup(_context: &Context) -> Env {
  // Clear whole screen (front buffer)
  gl::clear_color(0.0, 0.0, 0.0, 1.0);
  gl::clear(gl::GL_COLOR_BUFFER_BIT | gl::GL_DEPTH_BUFFER_BIT);

  gl_check();

  // Create a shader program
  let program = gl::create_program();
  gl_check();

  init_shader(program, gl::GL_VERTEX_SHADER, VERTEX_SHADER_SOURCE);

  init_shader(program, gl::GL_FRAGMENT_SHADER, FRAGMENT_SHADER_SOURCE);

  gl::link_program(program);
  print_program_info_log(program);
  gl_check();

  gl::use_program(program);
  gl_check();

  // Create Vertex Buffer Object
  let buffers = gl::gen_buffers(2);
  gl_check();
  let vertex_position_buffer = buffers[0];
  let vertex_color_buffer = buffers[1];

  let mut vertices: [gl::GLfloat; 9] = [0f32; 9];
  compute_triangle(&mut vertices);

  gl::bind_buffer(gl::GL_ARRAY_BUFFER, vertex_position_buffer);
  gl_check();
  gl::buffer_data(gl::GL_ARRAY_BUFFER, &vertices, gl::GL_STATIC_DRAW);
  gl_check();

  gl::bind_buffer(gl::GL_ARRAY_BUFFER, vertex_color_buffer);
  gl_check();
  gl::buffer_data(gl::GL_ARRAY_BUFFER, &VERTEX_COLOR, gl::GL_STATIC_DRAW);
  gl_check();

  // Get vertex attribute and uniform locations
  let vertex_position = gl::get_attrib_location(program, "vertexPosition");
  gl_check();
  if vertex_position < 0 {
    panic!("vertexPosition is negative ({})", vertex_position);
  }

  let vertex_color = gl::get_attrib_location(program, "vertexColor");
  gl_check();
  if vertex_color < 0 {
    panic!("vertexColor is negative ({})", vertex_color);
  }

  let projection_matrix = gl::get_uniform_location(program, "projectionMatrix");
  gl_check();
  let model_view_matrix = gl::get_uniform_location(program, "modelViewMatrix");
  gl_check();

  Env {
    vertex_position_buffer,
    vertex_color_buffer,
    vertex_position: vertex_position as gl::GLuint,
    vertex_color: vertex_color as gl::GLuint,
    projection_matrix,
    model_view_matrix,
    vertices,
  }
}

pub fn screen_capture(context: &Context) -> std::io::Result<()> {
  // Create buffer to hold entire front buffer pixels
  // We multiply width and height by 3 to because we use RGB!
  let width = (&context).width() as i32;
  let height = (&context).height() as i32;
  let size = (width * height * 4) as usize;
  let mut buffer: Vec<u8> = Vec::with_capacity(size);

  // Copy entire screen
  gl::read_pixels(
    0,                    /* x */
    0,                    /* y */
    width,                /* width */
    height,               /* height */
    gl::GL_RGBA,          /* format */
    gl::GL_UNSIGNED_BYTE, /* type_ */
    &mut buffer,          /* buffer */
  );
  gl_check();

  unsafe { buffer.set_len(size) };

  // Write all pixels to a file
  let mut output = File::create("triangle.raw")?;
  output.write_all(&buffer)?;

  Ok(())
}

pub fn triangle(context: &Context, env: &Env) {
  let (projection_matrix, model_view_matrix) = matrices((&context).width(), (&context).height());

  gl::uniform_matrix4fv(env.projection_matrix, false, &projection_matrix);
  gl_check();
  gl::uniform_matrix4fv(env.model_view_matrix, false, &model_view_matrix);
  gl_check();

  // Set vertex data - Positions
  gl::enable_vertex_attrib_array(env.vertex_position);
  gl_check();

  gl::bind_buffer(gl::GL_ARRAY_BUFFER, env.vertex_position_buffer);
  gl_check();

  gl::vertex_attrib_pointer_offset(
    env.vertex_position,                 /* index */
    3,                                   /* size */
    gl::GL_FLOAT,                        /* type */
    false,                               /* normalized */
    3 * size_of::<gl::GLfloat>() as i32, /* stride */
    0,                                   /* offset */
  );
  gl_check();

  // Colors
  gl::bind_buffer(gl::GL_ARRAY_BUFFER, env.vertex_color_buffer);
  gl_check();

  gl::vertex_attrib_pointer_offset(
    env.vertex_color,
    4,                                   /* num_components */
    gl::GL_FLOAT,                        /* type_ */
    false,                               /* normalize */
    0 * size_of::<gl::GLfloat>() as i32, /* stride */
    0,                                   /* offset */
  );
  gl_check();
  gl::enable_vertex_attrib_array(env.vertex_color);
  gl_check();

  // Render a triangle consisting of 3 vertices:
  gl::draw_arrays(gl::GL_TRIANGLES, 0, 3);
  gl_check();
}

// fn main() -> Result<(), Box<dyn Error>> {
fn main() -> Result<(), String> {
  let mut context = Context::new();

  // Set GL Viewport size, always needed!
  let desired_width = context.width() as i32;
  let desired_height = context.height() as i32;
  gl::viewport(0, 0, desired_width, desired_height);

  let (egl_major, egl_minor) = context.egl_version();
  println!("Initialized EGL version: {}.{}", egl_major, egl_minor);

  let viewport = mygl_get_viewport(gl::GL_VIEWPORT);
  println!("GL Viewport size: {}x{}", viewport[2], viewport[3]);
  // println!("GL Viewport size: {}x{}", context.width(), context.height());

  if viewport[2] != desired_width || viewport[3] != desired_height {
    return Err("Error! The glViewport returned incorrect values! Something is wrong!".to_string());
  }

  let env = setup(&context);

  triangle(&context, &env);

  // match screen_capture(&context) {
  //   Ok(_) => Ok(()),
  //   Err(_) => Err("Failed to open file triangle.raw for writing!"),
  // }?;

  context.swap_buffers();
  gl_check();

  thread::sleep(Duration::new(10, 0));

  Ok(())
}

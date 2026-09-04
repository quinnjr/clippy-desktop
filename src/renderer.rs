//! Draws the current frame into a transparent OpenGL window.
//!
//! The window is cleared to transparent black and one textured quad covering
//! the whole window is drawn with premultiplied-alpha blending, so everything
//! outside Clippy's outline stays see-through. Nearest-neighbour sampling
//! keeps the pixel art crisp at any integer scale.

use glow::{HasContext, PixelUnpackData};
use std::ffi::CStr;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlError(pub String);

impl fmt::Display for GlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OpenGL error: {}", self.0)
    }
}

impl std::error::Error for GlError {}

/// Full-window quad as a triangle strip: x, y, u, v per vertex. Texture row 0
/// is the top of the frame, so the top-left vertex samples v = 0.
#[rustfmt::skip]
const QUAD: [f32; 16] = [
    -1.0,  1.0, 0.0, 0.0,
    -1.0, -1.0, 0.0, 1.0,
     1.0,  1.0, 1.0, 0.0,
     1.0, -1.0, 1.0, 1.0,
];

const VERTEX_ES: &str = "#version 100
attribute vec2 a_pos;
attribute vec2 a_uv;
varying vec2 v_uv;
void main() {
    v_uv = a_uv;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}
";

const FRAGMENT_ES: &str = "#version 100
precision mediump float;
uniform sampler2D u_tex;
varying vec2 v_uv;
void main() {
    gl_FragColor = texture2D(u_tex, v_uv);
}
";

const VERTEX_LEGACY: &str = "#version 110
attribute vec2 a_pos;
attribute vec2 a_uv;
varying vec2 v_uv;
void main() {
    v_uv = a_uv;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}
";

const FRAGMENT_LEGACY: &str = "#version 110
uniform sampler2D u_tex;
varying vec2 v_uv;
void main() {
    gl_FragColor = texture2D(u_tex, v_uv);
}
";

const VERTEX_CORE: &str = "#version 150
in vec2 a_pos;
in vec2 a_uv;
out vec2 v_uv;
void main() {
    v_uv = a_uv;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}
";

const FRAGMENT_CORE: &str = "#version 150
uniform sampler2D u_tex;
in vec2 v_uv;
out vec4 o_color;
void main() {
    o_color = texture(u_tex, v_uv);
}
";

/// Picks shader dialects the current context accepts.
pub fn shader_sources(is_embedded: bool, major: u32, minor: u32) -> (&'static str, &'static str) {
    if is_embedded {
        (VERTEX_ES, FRAGMENT_ES)
    } else if (major, minor) >= (3, 2) {
        (VERTEX_CORE, FRAGMENT_CORE)
    } else {
        (VERTEX_LEGACY, FRAGMENT_LEGACY)
    }
}

pub struct Renderer {
    gl: glow::Context,
    program: glow::Program,
    vertex_array: Option<glow::VertexArray>,
    vertex_buffer: glow::Buffer,
    texture: glow::Texture,
    frame_width: u32,
    frame_height: u32,
}

impl Renderer {
    /// Loads OpenGL through `loader` and sets up the pipeline for frames of
    /// `frame_width` x `frame_height` pixels.
    ///
    /// # Safety
    ///
    /// A GL context must be current on this thread now and whenever any other
    /// method of the returned renderer is called, including its destructor.
    pub unsafe fn new<F>(loader: F, frame_width: u32, frame_height: u32) -> Result<Self, GlError>
    where
        F: FnMut(&CStr) -> *const std::ffi::c_void,
    {
        let gl = glow::Context::from_loader_function_cstr(loader);
        let version = gl.version();
        let (vertex_source, fragment_source) =
            shader_sources(version.is_embedded, version.major, version.minor);
        // Vertex array objects exist from OpenGL 3.0 and OpenGL ES 3.0 on, and
        // core profiles refuse to draw without one.
        let needs_vertex_array = version.major >= 3;

        let program = link_program(&gl, vertex_source, fragment_source)?;
        gl.use_program(Some(program));

        let vertex_array = if needs_vertex_array {
            let vertex_array = gl.create_vertex_array().map_err(GlError)?;
            gl.bind_vertex_array(Some(vertex_array));
            Some(vertex_array)
        } else {
            None
        };

        let vertex_buffer = gl.create_buffer().map_err(GlError)?;
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
        let quad_bytes: Vec<u8> = QUAD.iter().flat_map(|value| value.to_ne_bytes()).collect();
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &quad_bytes, glow::STATIC_DRAW);

        let stride = (4 * std::mem::size_of::<f32>()) as i32;
        let position = gl
            .get_attrib_location(program, "a_pos")
            .ok_or_else(|| GlError("shader has no a_pos attribute".to_string()))?;
        let uv = gl
            .get_attrib_location(program, "a_uv")
            .ok_or_else(|| GlError("shader has no a_uv attribute".to_string()))?;
        gl.vertex_attrib_pointer_f32(position, 2, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(position);
        gl.vertex_attrib_pointer_f32(
            uv,
            2,
            glow::FLOAT,
            false,
            stride,
            (2 * std::mem::size_of::<f32>()) as i32,
        );
        gl.enable_vertex_attrib_array(uv);

        let texture = gl.create_texture().map_err(GlError)?;
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::NEAREST as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::NEAREST as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            frame_width as i32,
            frame_height as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            PixelUnpackData::Slice(None),
        );
        let sampler = gl.get_uniform_location(program, "u_tex");
        gl.uniform_1_i32(sampler.as_ref(), 0);

        gl.disable(glow::DEPTH_TEST);
        gl.disable(glow::SCISSOR_TEST);
        gl.enable(glow::BLEND);
        // Premultiplied alpha: the frame texture is premultiplied and the
        // cleared background is transparent black.
        gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
        gl.clear_color(0.0, 0.0, 0.0, 0.0);

        check_gl_error(&gl, "renderer setup")?;

        Ok(Self {
            gl,
            program,
            vertex_array,
            vertex_buffer,
            texture,
            frame_width,
            frame_height,
        })
    }

    /// Matches the viewport to the window's physical size.
    pub fn resize(&self, width: u32, height: u32) {
        // SAFETY: the caller keeps the context current (see `new`).
        unsafe {
            self.gl.viewport(0, 0, width as i32, height as i32);
        }
    }

    /// Replaces the frame texture with `premultiplied_rgba`, which must hold
    /// `frame_width * frame_height * 4` bytes, top row first.
    pub fn upload(&self, premultiplied_rgba: &[u8]) -> Result<(), GlError> {
        let expected = (self.frame_width * self.frame_height * 4) as usize;
        if premultiplied_rgba.len() != expected {
            return Err(GlError(format!(
                "frame has {} bytes, expected {expected}",
                premultiplied_rgba.len()
            )));
        }
        // SAFETY: the caller keeps the context current (see `new`).
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            self.gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0,
                0,
                self.frame_width as i32,
                self.frame_height as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                PixelUnpackData::Slice(Some(premultiplied_rgba)),
            );
            check_gl_error(&self.gl, "frame upload")
        }
    }

    /// Clears to transparent and draws the frame over the whole window.
    pub fn draw(&self) {
        // SAFETY: the caller keeps the context current (see `new`).
        unsafe {
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            self.gl.use_program(Some(self.program));
            if let Some(vertex_array) = self.vertex_array {
                self.gl.bind_vertex_array(Some(vertex_array));
            }
            self.gl
                .bind_buffer(glow::ARRAY_BUFFER, Some(self.vertex_buffer));
            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        // SAFETY: the caller keeps the context current until the renderer is gone (see `new`).
        unsafe {
            self.gl.delete_texture(self.texture);
            self.gl.delete_buffer(self.vertex_buffer);
            if let Some(vertex_array) = self.vertex_array {
                self.gl.delete_vertex_array(vertex_array);
            }
            self.gl.delete_program(self.program);
        }
    }
}

unsafe fn link_program(
    gl: &glow::Context,
    vertex_source: &str,
    fragment_source: &str,
) -> Result<glow::Program, GlError> {
    let vertex = compile_shader(gl, glow::VERTEX_SHADER, vertex_source)?;
    let fragment = match compile_shader(gl, glow::FRAGMENT_SHADER, fragment_source) {
        Ok(fragment) => fragment,
        Err(err) => {
            gl.delete_shader(vertex);
            return Err(err);
        }
    };
    let program = gl.create_program().map_err(GlError)?;
    gl.attach_shader(program, vertex);
    gl.attach_shader(program, fragment);
    gl.link_program(program);
    gl.detach_shader(program, vertex);
    gl.detach_shader(program, fragment);
    gl.delete_shader(vertex);
    gl.delete_shader(fragment);
    if !gl.get_program_link_status(program) {
        let log = gl.get_program_info_log(program);
        gl.delete_program(program);
        return Err(GlError(format!("shader program failed to link: {log}")));
    }
    Ok(program)
}

unsafe fn compile_shader(
    gl: &glow::Context,
    kind: u32,
    source: &str,
) -> Result<glow::Shader, GlError> {
    let shader = gl.create_shader(kind).map_err(GlError)?;
    gl.shader_source(shader, source);
    gl.compile_shader(shader);
    if !gl.get_shader_compile_status(shader) {
        let log = gl.get_shader_info_log(shader);
        gl.delete_shader(shader);
        let what = if kind == glow::VERTEX_SHADER {
            "vertex"
        } else {
            "fragment"
        };
        return Err(GlError(format!("{what} shader failed to compile: {log}")));
    }
    Ok(shader)
}

unsafe fn check_gl_error(gl: &glow::Context, during: &str) -> Result<(), GlError> {
    let code = gl.get_error();
    if code == glow::NO_ERROR {
        Ok(())
    } else {
        Err(GlError(format!("code {code:#06x} during {during}")))
    }
}

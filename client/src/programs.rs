use std::collections::HashMap;
use std::rc::Rc;

use js_sys::Float32Array;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{
    HtmlImageElement, WebGlBuffer, WebGlProgram, WebGlShader, WebGlTexture, WebGlUniformLocation,
};

use scene::{Rect, Sprite, SpriteShape, SpriteVisual};

use crate::bridge::{log, Gl, JsError};

type Colour = [f32; 4];

// 0 is the default and what is used here
const GL_TEXTURE_DETAIL_LEVEL: i32 = 0;

// Required to be 0 for textures
const GL_TEXTURE_BORDER_WIDTH: i32 = 0;

struct Texture {
    pub width: u32,
    pub height: u32,
    pub texture: WebGlTexture,
}

impl Texture {
    fn new(gl: &Gl) -> Result<Texture, JsError> {
        Ok(Texture {
            width: 0,
            height: 0,
            texture: Texture::create_gl_texture(gl)?,
        })
    }

    fn gen_mipmap(&self, gl: &Gl) {
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.texture));
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MIN_FILTER, Gl::LINEAR as i32);
    }

    fn create_gl_texture(gl: &Gl) -> Result<WebGlTexture, JsError> {
        match gl.create_texture() {
            Some(t) => Ok(t),
            None => JsError::error("Unable to create texture."),
        }
    }

    fn load_u8_array(
        &mut self,
        gl: &Gl,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Result<(), JsError> {
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.texture));

        if gl
            .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                Gl::TEXTURE_2D,
                GL_TEXTURE_DETAIL_LEVEL,
                Gl::RGBA as i32,
                width as i32,
                height as i32,
                GL_TEXTURE_BORDER_WIDTH,
                Gl::RGBA,
                Gl::UNSIGNED_BYTE, // u8
                Some(data),
            )
            .is_err()
        {
            return JsError::error("Unable to load array.");
        }

        self.gen_mipmap(gl);

        self.width = width;
        self.height = height;

        Ok(())
    }

    fn from_u8_array(gl: &Gl, width: u32, height: u32, data: &[u8]) -> Result<Texture, JsError> {
        let mut texture = Texture::new(gl)?;
        texture.load_u8_array(gl, width, height, data)?;
        Ok(texture)
    }

    fn from_html_image(gl: &Gl, image: &HtmlImageElement) -> Result<Texture, JsError> {
        let mut texture = Texture::new(gl)?;
        texture.load_html_image(gl, image)?;

        Ok(texture)
    }

    fn load_html_image(&mut self, gl: &Gl, image: &HtmlImageElement) -> Result<(), JsError> {
        Texture::load_html_image_gl_texture(gl, image, &self.texture)?;
        self.width = image.natural_width();
        self.height = image.natural_height();
        self.gen_mipmap(gl);

        Ok(())
    }

    fn load_html_image_gl_texture(
        gl: &Gl,
        image: &HtmlImageElement,
        texture: &WebGlTexture,
    ) -> Result<(), JsError> {
        gl.bind_texture(Gl::TEXTURE_2D, Some(texture));

        if gl
            .tex_image_2d_with_u32_and_u32_and_html_image_element(
                Gl::TEXTURE_2D,
                GL_TEXTURE_DETAIL_LEVEL,
                Gl::RGBA as i32,
                Gl::RGBA,
                Gl::UNSIGNED_BYTE,
                image,
            )
            .is_err()
        {
            return JsError::error("Failed to create WebGL image.");
        }

        Ok(())
    }

    fn from_url(
        gl: &Gl,
        url: &str,
        callback: Box<dyn Fn(Result<Texture, JsError>)>,
    ) -> Result<(), JsError> {
        // Create HTML image to load image from url
        let image = match HtmlImageElement::new() {
            Ok(i) => Rc::new(i),
            Err(_) => return JsError::error("Unable to create image element."),
        };
        image.set_cross_origin(Some("")); // ?

        // Set callback to update texture once image is loaded
        {
            let gl = Rc::new(gl.clone());
            let image_ref = image.clone();
            let closure = Closure::wrap(Box::new(move || {
                callback(Texture::from_html_image(&gl, &image_ref));
            }) as Box<dyn FnMut()>);
            image.set_onload(Some(closure.as_ref().unchecked_ref()));
            closure.forget();
        }

        // Load image
        image.set_src(url);

        Ok(())
    }
}

struct TextureManager {
    gl: Rc<Gl>,
    textures: HashMap<scene::Id, Texture>,
    loading: Vec<scene::Id>,
}

impl TextureManager {
    fn new(gl: Rc<Gl>) -> Result<TextureManager, JsError> {
        let missing_texture = Texture::from_u8_array(&gl, 1, 1, &[0, 0, 255, 255])?;
        let mut tm = TextureManager {
            gl,
            textures: HashMap::new(),
            loading: Vec::new(),
        };
        tm.add_texture(0, missing_texture);
        Ok(tm)
    }

    fn load_image(&mut self, image: &HtmlImageElement) -> scene::Id {
        let id = match image.get_attribute("data-key") {
            Some(s) => parse_media_key(&s),
            None => 0,
        };

        if id != 0 {
            match Texture::from_html_image(&self.gl, image) {
                Ok(t) => self.textures.insert(id, t),
                Err(_) => return 0,
            };
        } else {
            log("Texture manager was asked to load texture without ID.");
        }

        id
    }

    // NB will overwrite existing texture of this id
    fn add_texture(&mut self, id: scene::Id, texture: Texture) {
        self.textures.insert(id, texture);
        self.loading.retain(|&i| i != id);
    }

    // Returns the requested texture, queueing it to load if necessary.
    // (yay side effects!)
    fn get_texture(&mut self, id: scene::Id) -> &WebGlTexture {
        if let Some(tex) = self.textures.get(&id) {
            &tex.texture
        } else {
            if !self.loading.contains(&id) {
                self.loading.push(id);
                crate::bridge::load_texture(format!("{id:016X}"));
            }

            // This unwrap is safe because we always add a missing texture
            // texture as id 0 in the constructor.
            &self.textures.get(&0).unwrap().texture
        }
    }
}

struct Shape {
    coords: Float32Array,
    position_buffer: WebGlBuffer,
    position_location: u32,
    matrix_location: WebGlUniformLocation,
    vertex_count: i32,
}

impl Shape {
    // Prebuild most shapes as there's no need to recompute common shapes every
    // time they're needed.
    const CIRCLE_EDGES: u32 = 32;
    const RECTANGLE: &'static [f32] = &[0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];

    // Requires that the program use "a_position" and "u_matrix"
    fn new(gl: &Gl, program: &WebGlProgram, points: &[f32]) -> Result<Self, JsError> {
        let coords = Float32Array::new_with_length(points.len() as u32);
        coords.copy_from(points);

        let position_location = gl.get_attrib_location(program, "a_position") as u32;
        let position_buffer = create_buffer(gl, Some(&coords))?;

        let matrix_location = get_uniform_location(gl, program, "u_matrix")?;

        let vertex_count = (coords.length() / 2) as i32;

        Ok(Shape {
            coords,
            position_buffer,
            position_location,
            matrix_location,
            vertex_count,
        })
    }

    // Returns a Shape for a regular polygon with n edges, or an error if the
    // program passed is non-functional.
    //
    // Note the resultant shape will be oriented with the first vertex at the
    // top center of the tile, i.e. a 4gon is a diamond and not a square.
    //
    // Based on:
    // https://webglfundamentals.org/webgl/lessons/webgl-drawing-without-data.html
    fn ngon(n: u32) -> Vec<f32> {
        let n_verts = n * 3;
        let r = 0.5;
        let mut coords = vec![];
        for i in 0..n_verts {
            let vert = i as f32;
            let slice = (vert / 3.0).floor();
            let pos = vert % 3.0;
            let edge = slice + pos;
            let theta = (edge / n as f32) * std::f32::consts::TAU;
            let radius = if pos > 1.5 { 0.0 } else { r };
            coords.push(theta.cos() * radius + 0.5);
            coords.push(theta.sin() * radius + 0.5);
        }

        coords
    }

    fn from_sprite_shape(
        gl: &Gl,
        program: &WebGlProgram,
        shape: SpriteShape,
    ) -> Result<Self, JsError> {
        match shape {
            SpriteShape::Ellipse => Self::new(gl, program, &Self::ngon(Self::CIRCLE_EDGES)),
            SpriteShape::Hexagon => Self::new(gl, program, &Self::ngon(6)),
            SpriteShape::Rectangle => Self::new(gl, program, Self::RECTANGLE),
            SpriteShape::Triangle => Self::new(gl, program, &Self::ngon(3)),
        }
    }

    // Should be called after using a program.
    fn draw(&self, gl: &Gl, vp: Rect, at: Rect) {
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.position_buffer));
        gl.enable_vertex_attrib_array(self.position_location);
        gl.vertex_attrib_pointer_with_i32(self.position_location, 2, Gl::FLOAT, false, 0, 0);

        let mut m = m4_orthographic(0.0, vp.w as f32, vp.h as f32, 0.0, -1.0, 1.0);
        m4_translate(&mut m, at.x - vp.x, at.y - vp.y, 0.0);
        m4_scale(&mut m, at.w, at.h, 1.0);

        gl.uniform_matrix4fv_with_f32_array(Some(&self.matrix_location), false, &m);
        gl.draw_arrays(Gl::TRIANGLES, 0, self.vertex_count);
    }
}

struct Shapes {
    ellipse: Shape,
    hexagon: Shape,
    rectangle: Shape,
    triangle: Shape,
}

impl Shapes {
    fn new(gl: &Gl, program: &WebGlProgram) -> Result<Self, JsError> {
        Ok(Shapes {
            ellipse: Shape::from_sprite_shape(gl, program, SpriteShape::Ellipse)?,
            hexagon: Shape::from_sprite_shape(gl, program, SpriteShape::Hexagon)?,
            rectangle: Shape::from_sprite_shape(gl, program, SpriteShape::Rectangle)?,
            triangle: Shape::from_sprite_shape(gl, program, SpriteShape::Triangle)?,
        })
    }

    fn shape(&self, shape: SpriteShape) -> &Shape {
        match shape {
            SpriteShape::Ellipse => &self.ellipse,
            SpriteShape::Hexagon => &self.hexagon,
            SpriteShape::Rectangle => &self.rectangle,
            SpriteShape::Triangle => &self.triangle,
        }
    }
}

struct SolidRenderer {
    gl: Rc<Gl>,
    program: WebGlProgram,
    colour_location: WebGlUniformLocation,
    shapes: Shapes,
}

impl SolidRenderer {
    fn new(gl: Rc<Gl>) -> Result<Self, JsError> {
        let program = create_program(
            &gl,
            include_str!("shaders/solid.vert"),
            include_str!("shaders/single.frag"),
        )?;

        let colour_location = get_uniform_location(&gl, &program, "u_color")?;
        let shapes = Shapes::new(&gl, &program)?;

        Ok(SolidRenderer {
            gl,
            program,
            colour_location,
            shapes,
        })
    }

    fn draw_shape(&self, shape: SpriteShape, colour: Colour, viewport: Rect, position: Rect) {
        let gl = &self.gl;

        gl.use_program(Some(&self.program));
        gl.uniform4fv_with_f32_array(Some(&self.colour_location), &colour);

        self.shapes.shape(shape).draw(gl, viewport, position);
    }
}

struct TextureRenderer {
    gl: Rc<Gl>,
    program: WebGlProgram,
    texcoord_buffer: WebGlBuffer,
    texcoord_location: u32,
    texture_location: WebGlUniformLocation,
    shapes: Shapes,
}

impl TextureRenderer {
    fn new(gl: Rc<Gl>) -> Result<Self, JsError> {
        let program = create_program(
            &gl,
            include_str!("shaders/solid.vert"),
            include_str!("shaders/image.frag"),
        )?;

        let shapes = Shapes::new(&gl, &program)?;

        let texcoord_location = gl.get_attrib_location(&program, "a_texcoord") as u32;
        let texcoord_buffer = create_buffer(&gl, Some(&shapes.rectangle.coords))?;
        let texture_location = get_uniform_location(&gl, &program, "u_texture")?;

        Ok(TextureRenderer {
            gl,
            program,
            texcoord_buffer,
            texcoord_location,
            texture_location,
            shapes,
        })
    }

    fn draw_texture(
        &self,
        shape: SpriteShape,
        texture: &WebGlTexture,
        viewport: Rect,
        position: Rect,
    ) {
        let gl = &self.gl;

        gl.bind_texture(Gl::TEXTURE_2D, Some(texture));
        gl.use_program(Some(&self.program));
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.texcoord_buffer));
        gl.enable_vertex_attrib_array(self.texcoord_location);
        gl.vertex_attrib_pointer_with_i32(self.texcoord_location, 2, Gl::FLOAT, false, 0, 0);

        gl.uniform1i(Some(&self.texture_location), 0);
        self.shapes.shape(shape).draw(gl, viewport, position);
    }
}

struct LineRenderer {
    gl: Rc<Gl>,
    program: WebGlProgram,
    position_location: u32,
    position_buffer: WebGlBuffer,
    colour_location: WebGlUniformLocation,
    point_count: i32,
}

impl LineRenderer {
    fn new(gl: Rc<Gl>) -> Result<LineRenderer, JsError> {
        let program = create_program(
            &gl,
            include_str!("shaders/line.vert"),
            include_str!("shaders/single.frag"),
        )?;
        let position_location = gl.get_attrib_location(&program, "a_position") as u32;
        let position_buffer = create_buffer(&gl, None)?;
        let colour_location = get_uniform_location(&gl, &program, "u_color")?;

        Ok(LineRenderer {
            gl,
            program,
            position_location,
            position_buffer,
            colour_location,
            point_count: 0,
        })
    }

    fn scale_and_load_points(&mut self, points: &mut [f32], vp_w: f32, vp_h: f32) {
        for (i, v) in points.iter_mut().enumerate() {
            // Point vectors are of form [x1, y1, x2, y2 ... xn, yn] so even indices are xs.
            if i % 2 == 0 {
                *v = to_unit(*v, vp_w);
            } else {
                *v = -to_unit(*v, vp_h);
            }
        }
        self.load_points(points);
    }

    fn load_points(&mut self, points: &[f32]) {
        let positions = Float32Array::from(points);

        self.gl
            .bind_buffer(Gl::ARRAY_BUFFER, Some(&self.position_buffer));
        self.gl.buffer_data_with_opt_array_buffer(
            Gl::ARRAY_BUFFER,
            Some(&positions.buffer()),
            Gl::STATIC_DRAW,
        );
        self.point_count = (points.len() / 2) as i32;
    }

    fn prepare_render(&self, colour: Option<Colour>) {
        let gl = &self.gl;

        gl.use_program(Some(&self.program));
        gl.enable_vertex_attrib_array(self.position_location);
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.position_buffer));
        gl.vertex_attrib_pointer_with_i32(self.position_location, 2, Gl::FLOAT, false, 0, 0);
        gl.uniform4fv_with_f32_array(
            Some(&self.colour_location),
            &colour.unwrap_or([0.5, 0.5, 0.5, 0.75]),
        );
    }

    fn render_lines(&self, colour: Option<Colour>) {
        self.prepare_render(colour);
        self.gl.draw_arrays(Gl::LINES, 0, self.point_count);
    }

    fn render_line_loop(&self, colour: Option<Colour>) {
        self.prepare_render(colour);
        self.gl.draw_arrays(Gl::LINE_LOOP, 0, self.point_count);
    }

    fn render_solid(&self, colour: Option<Colour>) {
        self.prepare_render(colour);
        self.gl.draw_arrays(Gl::TRIANGLES, 0, self.point_count);
    }
}

pub struct GridRenderer {
    line_renderer: LineRenderer,
    current_vp: Option<Rect>,
    current_grid_rect: Option<Rect>,
    current_grid_size: Option<f32>,
    current_line_count: Option<i32>,
}

impl GridRenderer {
    pub fn new(gl: Rc<Gl>) -> Result<GridRenderer, JsError> {
        Ok(GridRenderer {
            line_renderer: LineRenderer::new(gl)?,
            current_vp: None,
            current_grid_rect: None,
            current_grid_size: None,
            current_line_count: None,
        })
    }

    pub fn create_grid(&mut self, vp: Rect, dims: Rect, grid_size: f32) {
        let mut verticals = Vec::new();
        let mut horizontals = Vec::new();

        let d = grid_size;
        let dx = vp.x % grid_size;
        let dy = vp.y % grid_size;

        let w = vp.w;
        let h = vp.h;

        let sw = dims.w;
        let sh = dims.h;

        // Horizontal and vertical line start and endpoints, to ensure that we
        // render only the tiles that are part of the scene as part of the
        // grid.
        let fx = if vp.x < 0.0 {
            to_unit(vp.x.abs() / d, w / d)
        } else {
            -1.0
        };
        let tx = if (vp.x + w) / d > sw {
            to_unit(sw - vp.x / d, w / d).clamp(-1.0, 1.0)
        } else {
            1.0
        };
        let fy = if vp.y < 0.0 {
            -to_unit(vp.y.abs() / d, h / d)
        } else {
            1.0
        };
        let ty = if (vp.y + h) / d > sh {
            -to_unit(sh - vp.y / d, h / d).clamp(-1.0, 1.0)
        } else {
            -1.0
        };

        let mut i = 0.0;
        while i <= vp.w.max(vp.h) / d {
            let sx = i + (vp.x - dx) / d;
            let mut x = d * i - dx;
            if x <= w && sx >= 0.0 && sx <= sw {
                x = to_unit(x, w);

                verticals.push(x);
                verticals.push(fy);
                verticals.push(x);
                verticals.push(ty);
            }

            let sy = i + (vp.y - dy) / d;
            let mut y = d * i - dy;
            if y <= h && sy >= 0.0 && sy <= sh {
                // I negate the expression here but not for the x because the OpenGL coordinate system naturally matches
                // the browser coordinate system in the x direction, but opposes it in the y direction. By negating the
                // two coordinate systems are aligned, which makes things a little easier to work with.
                y = -to_unit(y, h);

                horizontals.push(fx);
                horizontals.push(y);
                horizontals.push(tx);
                horizontals.push(y);
            }

            i += 1.0;
        }

        verticals.append(&mut horizontals);
        self.line_renderer.load_points(&verticals);
        self.current_vp = Some(vp);
        self.current_grid_rect = Some(dims);
        self.current_grid_size = Some(grid_size);
        self.current_line_count = Some(verticals.len() as i32 / 2);
    }

    pub fn render_grid(&mut self, vp: Rect, dims: Rect, grid_size: f32) {
        if self.current_vp.is_none()
            || self.current_vp.unwrap() != vp
            || self.current_grid_rect.is_none()
            || self.current_grid_rect.unwrap() != dims
            || self.current_grid_size != Some(grid_size)
        {
            self.create_grid(vp, dims, grid_size);
        }

        self.line_renderer.render_lines(None);
    }
}

pub struct Renderer {
    // Loads and stores references to textures
    texture_library: TextureManager,

    solid_renderer: SolidRenderer,

    // Rendering program, used to draw sprites.
    texture_renderer: TextureRenderer,

    // To render outlines &c
    line_renderer: LineRenderer,

    // To render map grid
    grid_renderer: GridRenderer,
}

impl Renderer {
    pub fn new(gl: Rc<Gl>) -> Result<Renderer, JsError> {
        Ok(Renderer {
            texture_library: TextureManager::new(gl.clone())?,
            solid_renderer: SolidRenderer::new(gl.clone())?,
            texture_renderer: TextureRenderer::new(gl.clone())?,
            line_renderer: LineRenderer::new(gl.clone())?,
            grid_renderer: GridRenderer::new(gl)?,
        })
    }

    pub fn render_grid(&mut self, vp: Rect, dims: Rect, grid_size: f32) {
        self.grid_renderer.render_grid(vp, dims, grid_size);
    }

    pub fn load_image(&mut self, image: &HtmlImageElement) -> scene::Id {
        self.texture_library.load_image(image)
    }

    pub fn draw_sprite(&mut self, sprite: &Sprite, viewport: Rect, position: Rect) {
        match sprite.visual {
            SpriteVisual::Colour(colour) => {
                self.solid_renderer
                    .draw_shape(sprite.shape, colour, viewport, position)
            }
            SpriteVisual::Texture(id) => self.texture_renderer.draw_texture(
                sprite.shape,
                self.texture_library.get_texture(id),
                viewport,
                position,
            ),
        }
    }

    pub fn draw_outline(
        &mut self,
        Rect {
            x: vp_x,
            y: vp_y,
            w: vp_w,
            h: vp_h,
        }: Rect,
        Rect { x, y, w, h }: Rect,
    ) {
        self.line_renderer.scale_and_load_points(
            &mut [
                x - vp_x,
                y - vp_y,
                x - vp_x + w,
                y - vp_y,
                x - vp_x + w,
                y - vp_y + h,
                x - vp_x,
                y - vp_y + h,
            ],
            vp_w,
            vp_h,
        );
        self.line_renderer
            .render_line_loop(Some([0.5, 0.5, 1.0, 0.9]));
    }
}

fn create_shader(gl: &Gl, src: &str, stype: u32) -> Result<WebGlShader, JsError> {
    let shader = match gl.create_shader(stype) {
        Some(s) => s,
        None => return JsError::error("Failed to create shader."),
    };

    gl.shader_source(&shader, src);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, Gl::COMPILE_STATUS)
        .is_falsy()
    {
        return match gl.get_shader_info_log(&shader) {
            Some(_) => JsError::error("Shader compilation failed."),
            None => JsError::error("Shader compilation failed, no error message."),
        };
    }

    Ok(shader)
}

fn create_program(gl: &Gl, vert: &str, frag: &str) -> Result<WebGlProgram, JsError> {
    let program = match gl.create_program() {
        Some(p) => p,
        None => return JsError::error("WebGL program creation failed."),
    };

    gl.attach_shader(&program, &create_shader(gl, vert, Gl::VERTEX_SHADER)?);
    gl.attach_shader(&program, &create_shader(gl, frag, Gl::FRAGMENT_SHADER)?);

    gl.link_program(&program);

    if gl
        .get_program_parameter(&program, Gl::LINK_STATUS)
        .is_falsy()
    {
        gl.delete_program(Some(&program));
        return JsError::error("WebGL program linking failed.");
    }

    Ok(program)
}

fn create_buffer(gl: &Gl, data_opt: Option<&Float32Array>) -> Result<WebGlBuffer, JsError> {
    let buffer = match gl.create_buffer() {
        Some(b) => b,
        None => return JsError::error("Failed to create WebGL buffer."),
    };

    if let Some(data) = data_opt {
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buffer));
        gl.buffer_data_with_opt_array_buffer(
            Gl::ARRAY_BUFFER,
            Some(&data.buffer()),
            Gl::STATIC_DRAW,
        );
    }

    Ok(buffer)
}

fn get_uniform_location(
    gl: &Gl,
    program: &WebGlProgram,
    location: &str,
) -> Result<WebGlUniformLocation, JsError> {
    match gl.get_uniform_location(program, location) {
        Some(l) => Ok(l),
        None => Err(JsError::ResourceError(format!(
            "Failed to get WebGlUniformLocation {location}."
        ))),
    }
}

// Map value (as a proportion of scale) to [-1, 1]
fn to_unit(value: f32, scale: f32) -> f32 {
    ((2.0 * value) - scale) / scale
}

// see https://webglfundamentals.org/webgl/resources/m4.js
fn m4_orthographic(l: f32, r: f32, b: f32, t: f32, n: f32, f: f32) -> [f32; 16] {
    [
        2.0 / (r - l),
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 / (t - b),
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 / (n - f),
        0.0,
        (l + r) / (l - r),
        (b + t) / (b - t),
        (n + f) / (n - f),
        1.0,
    ]
}

// Translates matrix m by tx units in the x direction and likewise for ty and tz.
// NB: in place
fn m4_translate(m: &mut [f32; 16], tx: f32, ty: f32, tz: f32) {
    m[12] += m[0] * tx + m[4] * ty + m[8] * tz;
    m[13] += m[1] * tx + m[5] * ty + m[9] * tz;
    m[14] += m[2] * tx + m[6] * ty + m[10] * tz;
    m[15] += m[3] * tx + m[7] * ty + m[11] * tz;
}

// NB: in place
fn m4_scale(m: &mut [f32; 16], sx: f32, sy: f32, sz: f32) {
    m[0] *= sx;
    m[1] *= sx;
    m[2] *= sx;
    m[3] *= sx;
    m[4] *= sy;
    m[5] *= sy;
    m[6] *= sy;
    m[7] *= sy;
    m[8] *= sz;
    m[9] *= sz;
    m[10] *= sz;
    m[11] *= sz;
}

/// Parses a 16 digit hexadecimal media key string into an Id, reutrning 0
/// on failure.
pub fn parse_media_key(key: &str) -> scene::Id {
    if key.len() != 16 {
        return 0;
    }

    let mut raw = [0; 8];
    for (i, r) in raw.iter_mut().enumerate() {
        let j = i * 2;
        if let Ok(b) = u8::from_str_radix(&key[j..j + 2], 16) {
            *r = b;
        } else {
            return 0;
        }
    }

    i64::from_be_bytes(raw)
}

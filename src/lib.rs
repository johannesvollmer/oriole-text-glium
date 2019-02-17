use crate::font::Font;

use glium::Surface;
use glium::Program;
use glium::DrawError;
use glium::ProgramCreationError;
use glium::backend::Facade;
use glium::DrawParameters;
use glium::uniform;
use glium::texture::Texture2d;
use glium::texture::TextureCreationError;
use glium::texture::RawImage2d;
use std::borrow::Cow;
use glium::texture::ClientFormat;
use crate::atlas::Atlas;


pub struct SolidTextProgram {
    pub program: Program,
}

#[derive(Copy, Clone)]
pub struct GlyphQuadVertex {
    position: (f32, f32),
    texture_coordinate: (f32, f32),
}

pub struct TextMesh {
    vertices: glium::VertexBuffer<GlyphQuadVertex>,
    indices: glium::IndexBuffer<u16>,
    width: f32,
}

#[derive(Debug)]
pub enum TextMeshCreationError {
    Vertex(glium::vertex::BufferCreationError),
    Index(glium::index::BufferCreationError),
}


glium::implement_vertex!(GlyphQuadVertex, position, texture_coordinate);


pub fn atlas_texture(facade: &impl Facade, atlas: &Atlas)
                     -> Result<Texture2d, TextureCreationError>
{
    raw_u8_texture(facade, &atlas.distance_field, atlas.resolution)
}

pub fn raw_u8_texture(facade: &impl Facade, atlas: &[u8], dimensions: (usize, usize))
                      -> Result<Texture2d, TextureCreationError>
{
    glium::texture::Texture2d::new(
        facade, RawImage2d {
            data: Cow::Borrowed(atlas),
            width: dimensions.0 as u32,
            height: dimensions.1 as u32,
            format: ClientFormat::U8
        }
    )
}

impl SolidTextProgram {
    pub fn new(facade: &impl Facade) -> Result<Self, ProgramCreationError> {
        let program = glium::Program::from_source(
            facade,

            r#"version 330
                in vec2 position;
                in vec2 texture_coordinate;
                out vec2 texture_position;

                uniform mat4 transform;

                void main(){
                    gl_Position = (transform * vec4(position, 1.0, 1.0));
                    texture_position = texture_coordinate;
                }
            "#,

            r#"version 330
                in vec2 texture_position;
                out vec4 color;

                uniform vec4 fill;
                uniform texture2d distance_field;

                void main(){
                    float distance = texture(distance_field, texture_position).r;
                    distance = distance > 0.5? 1.0 : 0.0; // TODO

                    color = fill * vec4(vec3(1.0), distance);
                }
            "#,

            None
        );

        program.map(|program| SolidTextProgram { program })
    }

    pub fn draw(
        &self,
        surface: &mut impl Surface,
        font_distance_field: &glium::texture::Texture2d,
        mesh: &TextMesh,
        fill: (f32, f32, f32, f32),
        transform_matrix: [[f32; 4]; 4],
        draw_parameters: &DrawParameters,
    )
        -> Result<(), DrawError>
    {
        surface.draw(
            &mesh.vertices,
            &mesh.indices,
            &self.program,

            &uniform! {
                fill: fill,
                transform: transform_matrix,
                distance_field: font_distance_field,
            },

            draw_parameters
        )
    }
}

impl TextMesh {
    pub fn new(facade: &impl Facade, font: &Font, text: &str) -> Result<Self, TextMeshCreationError> {
        let (vertices, indices, width) = TextMesh::compute_buffers(font, text);

        Ok(TextMesh {
            vertices: glium::VertexBuffer::new(facade, &vertices)
                .map_err(|e| TextMeshCreationError::Vertex(e))?,

            indices: glium::IndexBuffer::new(facade, glium::index::PrimitiveType::TrianglesList, &indices)
                .map_err(|e| TextMeshCreationError::Index(e))?,

            width
        })
    }

    pub fn set(&mut self, font: &Font, text: &str){
        let (vertices, indices, width) = TextMesh::compute_buffers(font, text);
        self.vertices.write(&vertices);
        self.indices.write(&indices);
        self.width = width;
    }

    pub fn compute_buffers(font: &Font, text: &str) -> (Vec<GlyphQuadVertex>, Vec<u16>, f32) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut width = 0.0;

        for glyph in font.layout_glyphs(text.chars()) {
            let quad_positions = glyph.layout.in_mesh.vertices();
            let quad_texture_coords = glyph.layout.in_atlas.vertices();

            for quad_vertex_index in 0..4 {
                for triangle_index in &[ 0,1,2,  2,3,0 ] {
                    indices.push((vertices.len() + triangle_index) as u16);
                }

                width = glyph.layout.in_mesh.right();
                vertices.push(GlyphQuadVertex {
                    position: quad_positions[quad_vertex_index],
                    texture_coordinate: quad_texture_coords[quad_vertex_index]
                });
            }
        }

        (vertices, indices, width)
    }

    pub fn vertices(&self) -> &glium::VertexBuffer<GlyphQuadVertex> {
        &self.vertices
    }

    pub fn indices(&self) -> &glium::IndexBuffer<u16> {
        &self.indices
    }

    pub fn width(&self) -> f32 {
        self.width
    }
}

#[cfg(test)]
mod test {

    #[test]
    #[cfg(feature = "glium-render")]
    pub fn glium(font: crate::font::SerializedFont){
        use glium::{glutin, Surface};
        use crate::prelude::*;

        let mut events_loop = glutin::EventsLoop::new();
        let window = glutin::WindowBuilder::new();
        let context = glutin::ContextBuilder::new();
        let display = glium::Display::new(window, context, &events_loop).unwrap();

        let font = Font::deserialized(font);
        let font_texture = crate::glium_render::atlas_texture(&display, &font.atlas).unwrap();
        let text_mesh = crate::glium_render::TextMesh::new(&display, &font, "Hello World").unwrap();
        let solid_text_program = crate::glium_render::SolidTextProgram::new(&display).unwrap();

        let mut closed = false;
        while !closed {
            let mut target = display.draw();
            target.clear_color(0.0, 0.0, 0.1, 1.0);

            {
                let transform = [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ];

                let fill = (1.0, 0.8, 0.2, 1.0);
                let draw_parameters = glium::DrawParameters {
                    ..Default::default()
                };

                solid_text_program.draw(&mut target, &font_texture, &text_mesh, fill, transform, &draw_parameters).unwrap();
            }

            target.finish().unwrap();

            events_loop.poll_events(|ev| {
                match ev {
                    glutin::Event::WindowEvent { event, .. } => match event {
                        glutin::WindowEvent::CloseRequested => closed = true,
                        _ => (),
                    },
                    _ => (),
                }
            });
        }
    }
}
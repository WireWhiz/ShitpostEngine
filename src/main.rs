use crate::{graphics::Graphics, windowing::Window};

mod graphics;
mod windowing;

fn main() {
    // Boot up game
    let mut window = Window::new().expect("Unable to create window");

    let mut graphics =
        Graphics::new("Test app", Some(&window)).expect("Unable to initialize graphics");

    let main_shader =
        Graphics::compile_shader("shaders/triangle.slang").expect("Failed to compile shader");

    let main_mat = graphics
        .load_material(&main_shader)
        .expect("Failed to load main shader");
    println!("Created pipeline for main shader!");

    loop {
        window.update();
    }
}

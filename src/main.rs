use crate::{graphics::Graphics, windowing::Window};

mod graphics;
mod windowing;

fn main() {
    // Boot up game
    let mut window = Window::new().expect("Unable to create window");

    let mut graphics = Graphics::new("Test app").expect("Unable to initialize graphics");

    let surface = graphics.create_surface(&window);

    loop {
        window.update();
    }
}

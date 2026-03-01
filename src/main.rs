use crate::windowing::Window;

mod windowing;

fn main() {
    // Boot up game
    let mut window = Window::new().expect("Unable to create window");

    loop {
        window.update();
    }
}

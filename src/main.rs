use sss_scene_capture_lib::*;
use xcap::image;

fn main() {
    show_startup_prompt(&mut std::io::stdout()).unwrap();
    let mut reader = std::io::stdin().lock();
    wait_for_enter(&mut reader, || {
        countdown(&mut std::io::stdout(), std::thread::sleep).unwrap();
    })
    .unwrap();

    println!("Starting scene capture...");
    let image = capture_primary_monitor().unwrap();
    println!("Captured image: {}x{}", image.width(), image.height());
    image.save_with_format("./screen.png", image::ImageFormat::Png).unwrap();
}

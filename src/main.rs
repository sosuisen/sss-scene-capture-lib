use scene_capture::*;

fn main() {
    show_startup_prompt(&mut std::io::stdout()).unwrap();
    let mut reader = std::io::stdin().lock();
    wait_for_enter(&mut reader, || {
        countdown(&mut std::io::stdout(), std::thread::sleep).unwrap();
    })
    .unwrap();
}

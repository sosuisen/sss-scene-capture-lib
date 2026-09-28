use sss_scene_capture_lib::*;

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

    let now = chrono::Local::now();
    let session_dir = create_session_dir(now).unwrap();
    let path = session_dir.join("screen.png");
    save_image(&image, &path).unwrap();

    let recording = start_recording().unwrap();
    println!("Recording... Press Enter to stop.");
    wait_for_enter(&mut reader, || stop_recording(recording)).unwrap();
    println!("Stopped.");
}

fn create_session_dir(now: chrono::DateTime<chrono::Local>) -> std::io::Result<std::path::PathBuf> {
    let desktop =
        std::path::Path::new(&std::env::var("USERPROFILE").expect("USERPROFILE is not set"))
            .join("Desktop");
    let dir = desktop
        .join("sss-scene-capture")
        .join(now.format("%Y-%m-%d_%H-%M-%S").to_string());
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::path::Path;

    // デスクトップのsss-scene-captureフォルダの下に、日時名のフォルダが作られる。
    #[test]
    fn creates_datetime_named_folder_under_sss_scene_capture_on_desktop() {
        let desktop = Path::new(&std::env::var("USERPROFILE").unwrap()).join("Desktop");
        let now = chrono::Local
            .with_ymd_and_hms(2026, 9, 29, 12, 34, 56)
            .unwrap();

        let dir = create_session_dir(now).unwrap();

        assert_eq!(
            dir,
            desktop
                .join("sss-scene-capture")
                .join("2026-09-29_12-34-56")
        );
        assert!(dir.is_dir(), "{} was not created", dir.display());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

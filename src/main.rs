use sss_scene_capture_lib::*;
use std::io::BufRead;
use std::io::Write;

fn main() {
    show_startup_prompt(&mut std::io::stdout()).unwrap();
    let mut reader = std::io::stdin().lock();
    wait_for_enter(&mut reader, || {}).unwrap();

    println!("Starting scene capture...");
    let image = capture_primary_monitor().unwrap();
    println!("Captured image: {}x{}", image.width(), image.height());

    let now = chrono::Local::now();
    let session_dir = create_session_dir(now).unwrap();
    let path = session_dir.join("screen.png");

    let recording = start_recording().unwrap();
    println!("Recording... Press Enter to stop.");
    wait_for_enter(&mut reader, || stop_recording(recording)).unwrap();
    println!("Stopped.");

    save_image(&image, &path).unwrap();
}

fn show_startup_prompt(writer: &mut impl Write) -> std::io::Result<()> {
    writeln!(writer, "Press Enter to start...")?;
    Ok(())
}

fn wait_for_enter(reader: &mut impl BufRead, on_enter: impl FnOnce()) -> std::io::Result<()> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    if line.contains('\n') {
        on_enter();
    }
    Ok(())
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

    // 起動すると、Press Enter to start...が表示される。
    #[test]
    fn shows_press_enter_to_start_on_startup() {
        let mut buf: Vec<u8> = Vec::new();
        show_startup_prompt(&mut buf).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap().trim(),
            "Press Enter to start..."
        );
    }

    // 入力にEnterキーが含まれている場合、on_enterを呼び出す。
    #[test]
    fn calls_on_enter_when_enter_key_is_included_in_input() {
        let mut input: &[u8] = b"abc\n";
        let mut called = false;
        wait_for_enter(&mut input, || called = true).unwrap();
        assert!(called);
    }

    // 入力にEnterキーが含まれていない場合、on_enterを呼び出さない。
    #[test]
    fn does_not_call_on_enter_when_enter_key_is_not_included_in_input() {
        let mut input: &[u8] = b"abc";
        let mut called = false;
        wait_for_enter(&mut input, || called = true).unwrap();
        assert!(!called);
    }

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

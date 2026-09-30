use sss_scene_capture_lib::*;
use std::io::BufRead;
use std::io::Write;
use xcap::image::RgbaImage;

#[derive(Debug, PartialEq)]
enum AfterStopCommand {
    SaveAndNext,
    RetakePicture,
    Quit,
}

// Ctrl+Cまたはq+Enterで終了するまでセッションを繰り返す。入力が閉じられた（EOF）ときも終了する。
fn main() {
    let mut reader = std::io::stdin().lock();
    loop {
        show_startup_prompt(&mut std::io::stdout()).unwrap();
        if !wait_for_enter(&mut reader).unwrap() {
            break;
        }

        println!("Starting scene capture...");
        let image = capture_primary_monitor().unwrap();
        println!("Captured image: {}x{}", image.width(), image.height());

        println!("Recording sound... Press Enter to stop.");
        let Some((samples, sample_rate, channels)) = capture_sound(&mut reader) else {
            break;
        };
        println!("Stopped.");

        let Some(image) = confirm_image(&mut reader, image) else {
            break;
        };

        save(samples, sample_rate, channels, image);
    }
}

fn capture_sound(reader: &mut impl BufRead) -> Option<(Vec<f32>, u32, u16)> {
    let recording = start_recording().unwrap();
    if !wait_for_enter(reader).unwrap() {
        // 入力が閉じられた（EOF）ときはNoneを返す。
        return None;
    }
    let sample_rate = recording.sample_rate;
    let channels = recording.channels;
    let samples = stop_recording(recording);
    Some((samples, sample_rate, channels))
}

fn confirm_image(reader: &mut impl BufRead, mut image: RgbaImage) -> Option<RgbaImage> {
    loop {
        println!("Press Enter to save and next, s+Enter to retake picture, q+Enter to quit.");
        match read_after_stop_command(reader).unwrap() {
            AfterStopCommand::RetakePicture => {
                image = capture_primary_monitor().unwrap();
                println!("Retook image: {}x{}", image.width(), image.height());
            }
            AfterStopCommand::SaveAndNext => return Some(image),
            AfterStopCommand::Quit => return None,
        }
    }
}

fn save(samples: Vec<f32>, sample_rate: u32, channels: u16, image: RgbaImage) {
    let now = chrono::Local::now();
    let session_dir = create_session_dir(now).unwrap();
    let path = session_dir.join("screen.png");
    save_image(&image, &path).unwrap();
    save_mp3(
        &samples,
        sample_rate,
        channels,
        &session_dir.join("sound.mp3"),
    )
    .unwrap();
    println!("Saved to {}", session_dir.display());
}

fn show_startup_prompt(writer: &mut impl Write) -> std::io::Result<()> {
    writeln!(writer, "Press Enter to start...")?;
    Ok(())
}

// 1行読み、Enter（改行）が含まれていればtrueを返す。含まれないのは入力が閉じられたときだけである。
fn wait_for_enter(reader: &mut impl BufRead) -> std::io::Result<bool> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line.contains('\n'))
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

fn read_after_stop_command(reader: &mut impl BufRead) -> std::io::Result<AfterStopCommand> {
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if !line.contains('\n') {
            return Ok(AfterStopCommand::Quit);
        }
        match line.trim() {
            "" => return Ok(AfterStopCommand::SaveAndNext),
            "s" => return Ok(AfterStopCommand::RetakePicture),
            "q" => return Ok(AfterStopCommand::Quit),
            _ => {}
        }
    }
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

    // 入力にEnterキーが含まれている場合、trueを返す。
    #[test]
    fn returns_true_when_enter_key_is_included_in_input() {
        let mut input: &[u8] = b"abc\n";
        assert!(wait_for_enter(&mut input).unwrap());
    }

    // 入力にEnterキーが含まれていない場合、falseを返す。
    #[test]
    fn returns_false_when_enter_key_is_not_included_in_input() {
        let mut input: &[u8] = b"abc";
        assert!(!wait_for_enter(&mut input).unwrap());
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

    // 入力がs+Enterの場合、RetakePictureを返す。
    #[test]
    fn returns_retake_picture_when_input_is_s_plus_enter() {
        let mut input: &[u8] = b"s\n";
        assert_eq!(
            read_after_stop_command(&mut input).unwrap(),
            AfterStopCommand::RetakePicture
        );
    }

    // 入力がEnterのみの場合、SaveAndNextを返す。
    #[test]
    fn returns_save_and_next_when_input_is_enter_only() {
        let mut input: &[u8] = b"\n";
        assert_eq!(
            read_after_stop_command(&mut input).unwrap(),
            AfterStopCommand::SaveAndNext
        );
    }

    // 入力がq+Enterの場合、Quitを返す。
    #[test]
    fn returns_quit_when_input_is_q_plus_enter() {
        let mut input: &[u8] = b"q\n";
        assert_eq!(
            read_after_stop_command(&mut input).unwrap(),
            AfterStopCommand::Quit
        );
    }

    // 入力が閉じられている（EOF）場合、Quitを返す。
    #[test]
    fn returns_quit_when_input_is_closed() {
        let mut input: &[u8] = b"";
        assert_eq!(
            read_after_stop_command(&mut input).unwrap(),
            AfterStopCommand::Quit
        );
    }

    // 入力が未知の文字列のとき、無視して次の行を読む。
    #[test]
    fn ignores_unknown_input_and_reads_the_next_line() {
        let mut input: &[u8] = b"x\ns\n";
        assert_eq!(
            read_after_stop_command(&mut input).unwrap(),
            AfterStopCommand::RetakePicture
        );
    }
}

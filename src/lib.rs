use cpal::platform::Stream;
use cpal::traits::DeviceTrait;
use cpal::traits::HostTrait;
use cpal::traits::StreamTrait;
use std::io::BufRead;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use xcap::Monitor;
use xcap::image::RgbaImage;

pub struct Recording {
    pub stream: Stream,
    pub samples: Arc<Mutex<Vec<f32>>>,
    pub sample_rate: u32,
    pub channels: u16,
}

pub fn show_startup_prompt(writer: &mut impl Write) -> std::io::Result<()> {
    writeln!(writer, "Press Enter to start...")?;
    Ok(())
}

pub fn wait_for_enter(reader: &mut impl BufRead, on_enter: impl FnOnce()) -> std::io::Result<()> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    if line.contains('\n') {
        on_enter();
    }
    Ok(())
}

pub fn countdown(writer: &mut impl Write, mut sleep: impl FnMut(Duration)) -> std::io::Result<()> {
    for i in (1..=3).rev() {
        writeln!(writer, "{}...", i)?;
        sleep(Duration::from_secs(1));
    }
    writeln!(writer, "Go!")?;
    Ok(())
}

pub fn capture_primary_monitor() -> Result<RgbaImage, Box<dyn std::error::Error>> {
    let monitors = Monitor::all()?;
    let primary = monitors
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .ok_or("primary monitor not found")?;
    Ok(primary.capture_image()?)
}

pub fn start_recording() -> Result<Recording, Box<dyn std::error::Error>> {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&samples);

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("no output device available")?;
    let config = device.default_output_config()?.config();
    let stream = device.build_input_stream(
        config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            sink.lock().unwrap().extend_from_slice(data);
        },
        move |err| {
            eprintln!("Stream error: {:?}", err);
        },
        None,
    )?;
    stream.play()?;
    Ok(Recording {
        stream,
        samples,
        sample_rate: config.sample_rate,
        channels: config.channels,
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

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

    // 入力にEnterキーが含まれている場合、カウントダウン関数を呼び出す。
    #[test]
    fn calls_countdown_function_when_enter_key_is_included_in_input() {
        let mut input: &[u8] = b"abc\n";
        let mut called = false;
        wait_for_enter(&mut input, || called = true).unwrap();
        assert!(called);
    }

    // 入力にEnterキーが含まれていない場合、カウントダウン関数を呼び出さない。
    #[test]
    fn does_not_call_countdown_function_when_enter_key_is_not_included_in_input() {
        let mut input: &[u8] = b"abc";
        let mut called = false;
        wait_for_enter(&mut input, || called = true).unwrap();
        assert!(!called);
    }

    // カウントダウン関数を呼び出すと、3秒間のカウントダウンが表示される。
    #[test]
    fn displays_countdown_when_countdown_function_is_called() {
        let mut buf: Vec<u8> = Vec::new();
        countdown(&mut buf, |_| {}).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap().trim(),
            "3...\n2...\n1...\nGo!"
        );
    }

    // カウントダウンは1秒おきに減る
    #[test]
    fn countdown_decreases_every_second() {
        let mut buf: Vec<u8> = Vec::new();
        let mut sleeps: Vec<Duration> = Vec::new();
        countdown(&mut buf, |d| sleeps.push(d)).unwrap();
        assert_eq!(sleeps, vec![Duration::from_secs(1); 3]);
    }

    // プライマリーモニターをキャプチャしたら、高さと幅が100より大きい画像が返ってくる。
    #[test]
    fn captures_primary_monitor_and_returns_image_with_height_and_width_greater_than_100() {
        let image = capture_primary_monitor().unwrap();
        assert!(image.width() > 100);
        assert!(image.height() > 100);
    }

    // 録音開始ルーチンは、Streamを返す。
    #[test]
    fn start_recording_routine_returns_stream() {
        let result: Result<Recording, Box<dyn std::error::Error>> = start_recording();
        assert!(result.is_ok());
    }

    // 録音ルーチンを1秒間実行すると、1秒分の録音データが生成される。
    #[test]
    fn start_recording_routine_generates_1_second_of_audio_data() {
        let _tone = play_test_tone().unwrap();
        let recording = start_recording().unwrap();
        std::thread::sleep(Duration::from_secs(1));
        drop(recording.stream);

        let expected = recording.sample_rate as usize * recording.channels as usize;
        let actual = recording.samples.lock().unwrap().len();
        let tolerance = expected / 5;
        assert!(
            (actual as i64 - expected as i64).abs() <= tolerance as i64,
            "Expected about {expected} samples, but got {actual} samples"
        );
    }

    // テスト用に、既定の出力デバイスで440Hzの正弦波を小さな音量で鳴らし続ける。
    fn play_test_tone() -> Result<Stream, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("no output device available")?;
        let config = device.default_output_config()?.config();
        let sample_rate = config.sample_rate as f32;
        let channels = config.channels as usize;
        let mut phase: f32 = 0.0;
        let stream = device.build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for frame in data.chunks_mut(channels) {
                    let value = (phase * 2.0 * std::f32::consts::PI).sin() * 0.001;
                    phase = (phase + 440.0 / sample_rate) % 1.0;
                    for sample in frame {
                        *sample = value;
                    }
                }
            },
            |err| eprintln!("Tone error: {:?}", err),
            None,
        )?;
        stream.play()?;
        Ok(stream)
    }
}

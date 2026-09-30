use cpal::platform::Stream;
use cpal::traits::DeviceTrait;
use cpal::traits::HostTrait;
use cpal::traits::StreamTrait;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::Sender;
use std::sync::mpsc::channel;
use std::thread::JoinHandle;
use xcap::Monitor;
use xcap::image::RgbaImage;

// 録音セッション。cpalのStreamは録音専用スレッドだけが所有する（ADR-003）。
// 外に出るのは停止命令を送るSenderと、スレッドの終了を待つJoinHandleだけで、どちらもSendである。
pub struct Recording {
    pub samples: Arc<Mutex<Vec<f32>>>,
    pub sample_rate: u32,
    pub channels: u16,
    stop_tx: Sender<()>,
    thread: JoinHandle<()>,
}

pub fn capture_primary_monitor() -> Result<RgbaImage, Box<dyn std::error::Error>> {
    let monitors = Monitor::all()?;
    let primary = monitors
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .ok_or("primary monitor not found")?;
    Ok(primary.capture_image()?)
}

pub fn save_image(image: &RgbaImage, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    image.save(path)?;
    Ok(())
}

pub fn start_recording() -> Result<Recording, Box<dyn std::error::Error>> {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&samples);
    let (ready_tx, ready_rx) = channel();
    let (stop_tx, stop_rx) = channel::<()>();

    let thread = std::thread::spawn(move || match build_streams(sink) {
        Ok((stream, silence, config)) => {
            ready_tx
                .send(Ok((config.sample_rate, config.channels)))
                .unwrap();
            // 停止命令が来るか、Senderが捨てられるまで待つ。
            let _ = stop_rx.recv();
            drop(stream);
            drop(silence);
        }
        Err(e) => ready_tx.send(Err(e)).unwrap(),
    });

    let (sample_rate, channels) = ready_rx
        .recv()
        .map_err(|_| "recording thread exited before it was ready")??;

    Ok(Recording {
        samples,
        sample_rate,
        channels,
        stop_tx,
        thread,
    })
}

// 既定の再生デバイスでループバック録音ストリームと無音の出力ストリームを作り、両方を開始する。
// エラーはスレッド境界を越えられるようStringで返す。
fn build_streams(
    sink: Arc<Mutex<Vec<f32>>>,
) -> Result<(Stream, Stream, cpal::StreamConfig), String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("no output device available")?;
    let config = device
        .default_output_config()
        .map_err(|e| e.to_string())?
        .config();

    let mut started = false;
    let stream = device
        .build_input_stream(
            config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let data = if started {
                    data
                } else {
                    match data.iter().position(|s| *s != 0.0) {
                        Some(i) => {
                            started = true;
                            &data[i..]
                        }
                        None => return,
                    }
                };
                sink.lock().unwrap().extend_from_slice(data);
            },
            move |err| {
                eprintln!("Stream error: {:?}", err);
            },
            None,
        )
        .map_err(|e| e.to_string())?;
    let silence = device
        .build_output_stream(
            config,
            |data: &mut [f32], _: &cpal::OutputCallbackInfo| data.fill(0.0),
            |err| eprintln!("Silence error: {:?}", err),
            None,
        )
        .map_err(|e| e.to_string())?;
    silence.play().map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, silence, config))
}

pub fn stop_recording(recording: Recording) -> Vec<f32> {
    // 送信もjoinも、相手のスレッドがすでに終わっているときだけ失敗する。無視してよい。
    let _ = recording.stop_tx.send(());
    let _ = recording.thread.join();
    std::mem::take(&mut *recording.samples.lock().unwrap())
}

pub fn save_wav(
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &sample in samples {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;
    Ok(())
}

pub fn save_mp3(
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = rusty_mp3::Mp3EncoderConfig {
        bitrate_kbps: 192,
        vbr_quality: None, // NoneでCBR
    };
    let mut encoder = rusty_mp3::Mp3Encoder::new(config);
    encoder.push_pcm_f32(samples, channels, sample_rate)?;
    encoder.finish();

    let mut mp3 = Vec::new();
    loop {
        match encoder.next_packet() {
            Ok(frame) => mp3.extend_from_slice(&frame),
            Err(rusty_mp3::error::Error::Eof) => break,
            Err(e) => return Err(e.into()),
        }
    }
    std::fs::write(path, mp3)?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use std::time::Duration;

    use super::*;

    pub(crate) static AUDIO_DEVICE: Mutex<()> = Mutex::new(());

    // プライマリーモニターをキャプチャしたら、高さと幅が100より大きい画像が返ってくる。
    #[test]
    fn captures_primary_monitor_and_returns_image_with_height_and_width_greater_than_100() {
        let image = capture_primary_monitor().unwrap();
        assert!(image.width() > 100);
        assert!(image.height() > 100);
    }

    // 画像が指定のファイル名で保存される。
    #[test]
    fn saves_image_to_the_given_file_name() {
        let dir = std::env::temp_dir().join("sss_saves_image_to_the_given_file_name");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("screen.png");
        let image = RgbaImage::new(2, 2);

        save_image(&image, &path).unwrap();

        assert!(path.exists(), "{} was not created", path.display());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // 録音開始ルーチンは、Recordingを返す。
    #[test]
    fn start_recording_routine_returns_recording() {
        let result: Result<Recording, Box<dyn std::error::Error>> = start_recording();
        assert!(result.is_ok());
    }

    // 録音ルーチンを1秒間実行すると、1秒分の録音データが生成される。
    #[test]
    fn start_recording_routine_generates_1_second_of_audio_data() {
        let _audio = AUDIO_DEVICE.lock().unwrap_or_else(|e| e.into_inner());

        let _tone = play_test_tone().unwrap();
        let recording = start_recording().unwrap();
        let expected = recording.sample_rate as usize * recording.channels as usize;
        std::thread::sleep(Duration::from_secs(1));
        let samples = stop_recording(recording);

        let actual = samples.len();
        let tolerance = expected / 5;
        assert!(
            actual.abs_diff(expected) <= tolerance,
            "Expected about {expected} samples, but got {actual} samples"
        );
    }

    // 録音開始直後の無音は記録されない。最初に音が届いた時点から記録が始まる。
    #[test]
    fn start_recording_routine_does_not_record_silence_at_the_beginning() {
        let _audio = AUDIO_DEVICE.lock().unwrap_or_else(|e| e.into_inner());

        let _silence = play_silence().unwrap();
        let recording = start_recording().unwrap();
        let expected = recording.sample_rate as usize * recording.channels as usize;
        std::thread::sleep(Duration::from_millis(500));
        let _tone = play_test_tone().unwrap();
        std::thread::sleep(Duration::from_millis(1000));
        let samples = stop_recording(recording);

        assert_ne!(samples[0], 0.0, "recording started with silence");

        let actual = samples.len();
        let tolerance = expected / 5;
        assert!(
            actual.abs_diff(expected) <= tolerance,
            "Expected about {expected} samples, but got {actual} samples"
        );
    }

    // 録音中に無音の区間があっても、その区間のデータが記録される。
    #[test]
    fn start_recording_routine_records_silence_in_the_middle() {
        let _audio = AUDIO_DEVICE.lock().unwrap_or_else(|e| e.into_inner());

        let recording = start_recording().unwrap();
        let expected = recording.sample_rate as usize * recording.channels as usize * 3 / 2;
        let tone = play_test_tone().unwrap();
        std::thread::sleep(Duration::from_millis(500));
        drop(tone); // ここから無音
        std::thread::sleep(Duration::from_millis(500));
        let _tone = play_test_tone().unwrap(); // 再び音
        std::thread::sleep(Duration::from_millis(500));
        let samples = stop_recording(recording);

        let actual = samples.len();
        let tolerance = expected / 5;
        assert!(
            actual.abs_diff(expected) <= tolerance,
            "Expected about {expected} samples (1.5 seconds), but got {actual} samples"
        );
    }

    // 録音停止ルーチンを呼び出すと、録音が停止する。
    #[test]
    fn stop_recording_routine_stops_recording() {
        let _audio = AUDIO_DEVICE.lock().unwrap_or_else(|e| e.into_inner());

        let _tone = play_test_tone().unwrap();
        let recording = start_recording().unwrap();
        let samples = Arc::clone(&recording.samples);
        std::thread::sleep(Duration::from_millis(500));

        stop_recording(recording);
        let count_at_stop = samples.lock().unwrap().len();
        std::thread::sleep(Duration::from_millis(500));

        assert_eq!(samples.lock().unwrap().len(), count_at_stop);
    }

    // 音声が指定のファイル名で保存される。
    #[test]
    fn saves_wav_to_the_given_file_name() {
        let dir = std::env::temp_dir().join("sss_saves_wav_to_the_given_file_name");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sound.wav");
        let samples = [0.0f32, 0.5, -0.5, 0.0];

        save_wav(&samples, 48000, 2, &path).unwrap();

        assert!(path.exists(), "{} was not created", path.display());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // 音声がMP3として指定のファイル名で保存される。
    #[test]
    fn saves_mp3_to_the_given_file_name() {
        let dir = std::env::temp_dir().join("sss_saves_mp3_to_the_given_file_name");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sound.mp3");
        let samples = [0.0f32, 0.5, -0.5, 0.0];

        save_mp3(&samples, 48000, 2, &path).unwrap();

        assert!(path.exists(), "{} was not created", path.display());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // 録音停止ルーチンは、録音したサンプル列を返す。
    #[test]
    fn stop_recording_routine_returns_recorded_samples() {
        let _audio = AUDIO_DEVICE.lock().unwrap_or_else(|e| e.into_inner());

        let _tone = play_test_tone().unwrap();
        let recording = start_recording().unwrap();
        let expected = recording.sample_rate as usize * recording.channels as usize / 2;
        std::thread::sleep(Duration::from_millis(500));

        let samples = stop_recording(recording);

        assert!(
            samples.len().abs_diff(expected) <= expected / 5,
            "Expected about {expected} samples (0.5 seconds), but got {}",
            samples.len()
        );
    }

    pub(crate) fn play_test_tone() -> Result<Stream, Box<dyn std::error::Error>> {
        play_sine(0.001)
    }

    fn play_silence() -> Result<Stream, Box<dyn std::error::Error>> {
        play_sine(0.0)
    }

    fn play_sine(amplitude: f32) -> Result<Stream, Box<dyn std::error::Error>> {
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
                    let value = (phase * 2.0 * std::f32::consts::PI).sin() * amplitude;
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

pub mod ffi;

// 別言語向けのC ABI（ADR-003、ADR-005）。ネイティブAPIを呼ぶ薄い翻訳層である。

use crate::Recording;
use crate::capture_primary_monitor;
use crate::save_image;
use crate::save_mp3;
use crate::save_wav;
use crate::start_recording;
use crate::stop_recording;
use std::collections::HashMap;
use std::ffi::CStr;
use std::ffi::c_char;
use std::path::Path;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

// 録音のレジストリ。所有権はここにあり、呼び出し側はIDの数値だけを持つ。
static RECORDINGS: LazyLock<Mutex<HashMap<u64, Recording>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// IDは1から単調増加で、再利用しない。0は失敗を表すために予約する。
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

// 戻り値のコード。0が成功で、それ以外は失敗の理由を表す。
pub const SSS_OK: i32 = 0;
pub const SSS_ERR_INVALID_ID: i32 = 1;
pub const SSS_ERR_INVALID_PATH: i32 = 2;
pub const SSS_ERR_INVALID_FORMAT: i32 = 3;
pub const SSS_ERR_SAVE_FAILED: i32 = 4;
pub const SSS_ERR_CAPTURE_FAILED: i32 = 5;
// Rust側のpanicを境界で止めたときのコード。panicがextern "C"を越えるのは未定義動作である。
pub const SSS_ERR_PANIC: i32 = 99;

// 保存形式のコード。
pub const SSS_FORMAT_WAV: i32 = 0;
pub const SSS_FORMAT_MP3: i32 = 1;

// 録音を始め、IDを返す。失敗したら0を返す。
#[unsafe(no_mangle)]
pub extern "C" fn sss_start_recording() -> u64 {
    std::panic::catch_unwind(|| {
        let Ok(recording) = start_recording() else {
            return 0;
        };
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        RECORDINGS.lock().unwrap().insert(id, recording);
        id
    })
    .unwrap_or(0)
}

// IDの録音を止め、formatの形式でpathに保存する。
#[unsafe(no_mangle)]
pub extern "C" fn sss_stop_recording(id: u64, format: i32, path: *const c_char) -> i32 {
    std::panic::catch_unwind(|| {
        // 引数の検査は録音を取り出す前に行う。不正な引数では録音を止めない。
        let save: SaveFn = match format {
            SSS_FORMAT_WAV => save_wav,
            SSS_FORMAT_MP3 => save_mp3,
            _ => return SSS_ERR_INVALID_FORMAT,
        };
        let Some(path) = path_from_c(path) else {
            return SSS_ERR_INVALID_PATH;
        };
        let Some(recording) = RECORDINGS.lock().unwrap().remove(&id) else {
            return SSS_ERR_INVALID_ID;
        };
        let sample_rate = recording.sample_rate;
        let channels = recording.channels;
        let samples = stop_recording(recording);
        match save(&samples, sample_rate, channels, path) {
            Ok(()) => SSS_OK,
            Err(_) => SSS_ERR_SAVE_FAILED,
        }
    })
    .unwrap_or(SSS_ERR_PANIC)
}

// プライマリーモニターのスクリーンショットを撮り、PNGとしてpathに保存する。
#[unsafe(no_mangle)]
pub extern "C" fn sss_capture_primary_monitor(path: *const c_char) -> i32 {
    std::panic::catch_unwind(|| {
        let Some(path) = path_from_c(path) else {
            return SSS_ERR_INVALID_PATH;
        };
        let Ok(image) = capture_primary_monitor() else {
            return SSS_ERR_CAPTURE_FAILED;
        };
        match save_image(&image, path) {
            Ok(()) => SSS_OK,
            Err(_) => SSS_ERR_SAVE_FAILED,
        }
    })
    .unwrap_or(SSS_ERR_PANIC)
}

type SaveFn = fn(&[f32], u32, u16, &Path) -> Result<(), Box<dyn std::error::Error>>;

// C文字列をパスにする。NULLかUTF-8でなければNoneを返す。
// NULLでないポインタは、呼び出し側がNUL終端の文字列を渡している契約で読む。
fn path_from_c<'a>(path: *const c_char) -> Option<&'a Path> {
    if path.is_null() {
        return None;
    }
    let path = unsafe { CStr::from_ptr(path) }.to_str().ok()?;
    Some(Path::new(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::AUDIO_DEVICE;
    use crate::tests::play_test_tone;
    use std::ffi::CString;
    use std::time::Duration;

    // sss_start_recordingは、0以外のIDを返す。
    #[test]
    fn sss_start_recording_returns_non_zero_id() {
        let _audio = AUDIO_DEVICE.lock().unwrap_or_else(|e| e.into_inner());

        let id = sss_start_recording();

        assert_ne!(id, 0);
        // 後片付け。録音を止めてデバイスを返す。
        let recording = RECORDINGS.lock().unwrap().remove(&id).unwrap();
        stop_recording(recording);
    }

    // 無効なIDでsss_stop_recordingを呼ぶと、SSS_ERR_INVALID_IDを返す。
    #[test]
    fn sss_stop_recording_returns_invalid_id_error_for_unknown_id() {
        let path = CString::new("unused.wav").unwrap();
        let code = sss_stop_recording(u64::MAX, SSS_FORMAT_WAV, path.as_ptr());
        assert_eq!(code, SSS_ERR_INVALID_ID);
    }

    // sss_stop_recordingは、形式コードWAVで指定パスにファイルを書き、SSS_OKを返す。
    #[test]
    fn sss_stop_recording_writes_wav_to_the_given_path() {
        let path = std::env::temp_dir().join("sss_ffi_test_stop.wav");
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        let _audio = AUDIO_DEVICE.lock().unwrap_or_else(|e| e.into_inner());
        let id = sss_start_recording();

        let code = sss_stop_recording(id, SSS_FORMAT_WAV, c_path.as_ptr());

        assert_eq!(code, SSS_OK);
        assert!(path.is_file(), "{} was not written", path.display());
        std::fs::remove_file(&path).unwrap();
    }

    // sss_stop_recordingは、形式コードMP3で指定パスにファイルを書き、SSS_OKを返す。
    #[test]
    fn sss_stop_recording_writes_mp3_to_the_given_path() {
        let path = std::env::temp_dir().join("sss_ffi_test_stop.mp3");
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        let _audio = AUDIO_DEVICE.lock().unwrap_or_else(|e| e.into_inner());
        let _tone = play_test_tone().unwrap();
        let id = sss_start_recording();
        std::thread::sleep(Duration::from_millis(500));

        let code = sss_stop_recording(id, SSS_FORMAT_MP3, c_path.as_ptr());

        assert_eq!(code, SSS_OK);
        let bytes = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        // MP3フレームはFF Fx（同期ワード）で始まる。WAVならRIFFで始まる。
        assert_eq!(bytes[0], 0xFF, "not an MP3 frame: {:02X?}", &bytes[..4.min(bytes.len())]);
    }

    // 同じIDで2回目のsss_stop_recordingを呼ぶと、SSS_ERR_INVALID_IDを返す。
    #[test]
    fn sss_stop_recording_returns_invalid_id_error_on_second_call() {
        let path = std::env::temp_dir().join("sss_ffi_test_stop_twice.wav");
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        let _audio = AUDIO_DEVICE.lock().unwrap_or_else(|e| e.into_inner());
        let id = sss_start_recording();
        assert_eq!(sss_stop_recording(id, SSS_FORMAT_WAV, c_path.as_ptr()), SSS_OK);
        std::fs::remove_file(&path).unwrap();

        let code = sss_stop_recording(id, SSS_FORMAT_WAV, c_path.as_ptr());

        assert_eq!(code, SSS_ERR_INVALID_ID);
        assert!(!path.exists(), "second call must not write a file");
    }

    // 不正な形式コードを渡すと、SSS_ERR_INVALID_FORMATを返す。録音は止めない。
    #[test]
    fn sss_stop_recording_returns_invalid_format_error_and_keeps_recording() {
        let path = std::env::temp_dir().join("sss_ffi_test_bad_format.wav");
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        let _audio = AUDIO_DEVICE.lock().unwrap_or_else(|e| e.into_inner());
        let id = sss_start_recording();

        let code = sss_stop_recording(id, 99, c_path.as_ptr());

        assert_eq!(code, SSS_ERR_INVALID_FORMAT);
        assert!(!path.exists(), "invalid format must not write a file");
        // 録音は生きているので、正しい形式で止められる。
        assert_eq!(sss_stop_recording(id, SSS_FORMAT_WAV, c_path.as_ptr()), SSS_OK);
        std::fs::remove_file(&path).unwrap();
    }

    // パスがNULLのとき、SSS_ERR_INVALID_PATHを返す。録音は止めない。
    #[test]
    fn sss_stop_recording_returns_invalid_path_error_for_null_path() {
        let _audio = AUDIO_DEVICE.lock().unwrap_or_else(|e| e.into_inner());
        let id = sss_start_recording();

        let code = sss_stop_recording(id, SSS_FORMAT_WAV, std::ptr::null());

        assert_eq!(code, SSS_ERR_INVALID_PATH);
        let path = std::env::temp_dir().join("sss_ffi_test_null_path.wav");
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        assert_eq!(sss_stop_recording(id, SSS_FORMAT_WAV, c_path.as_ptr()), SSS_OK);
        std::fs::remove_file(&path).unwrap();
    }

    // sss_capture_primary_monitorは、指定パスにPNGを書き、SSS_OKを返す。
    #[test]
    fn sss_capture_primary_monitor_writes_png_to_the_given_path() {
        let path = std::env::temp_dir().join("sss_ffi_test_screen.png");
        let c_path = CString::new(path.to_str().unwrap()).unwrap();

        let code = sss_capture_primary_monitor(c_path.as_ptr());

        assert_eq!(code, SSS_OK);
        let bytes = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(&bytes[..4], [0x89, b'P', b'N', b'G'], "not a PNG file");
    }

    // sss_capture_primary_monitorは、パスがNULLのときSSS_ERR_INVALID_PATHを返す。
    #[test]
    fn sss_capture_primary_monitor_returns_invalid_path_error_for_null_path() {
        assert_eq!(sss_capture_primary_monitor(std::ptr::null()), SSS_ERR_INVALID_PATH);
    }
}

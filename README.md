# sss-scene-capture-lib

アプリの画面とサウンドを記録するWindows専用のCLIツール、およびそのライブラリ。

1回のセッションにつき、開始時のプライマリーモニターを1枚の静止画として保存する。
開始から終了までのPCのシステムサウンドをMP3として保存する。

## 動作環境

- Windows 10以降
- Rust 2024 edition

システムサウンドはWASAPIのループバック録音で取得する。マイクは録音しない。

## 使い方

```
cargo run --release
```

1. `Press Enter to start...`と表示されたらEnterキーを押す。
2. スクリーンショットが撮られ、録音が始まる。
3. もう一度Enterキーを押すと録音が止まり、ファイルが保存される。
4. 次のセッションが始まる。Ctrl+Cで終了する。

保存先は次のフォルダである。

```
%USERPROFILE%\Desktop\sss-scene-capture\YYYY-MM-DD_HH-MM-SS\
  screen.png
  sound.mp3
```

## ライブラリとして使う

`src/lib.rs`は次の関数を公開している。

| 関数 | 役割 |
| --- | --- |
| `capture_primary_monitor()` | プライマリーモニターのスクリーンショットを撮る |
| `save_image(image, path)` | 画像をファイルに保存する |
| `start_recording()` | システムサウンドの録音を始める |
| `stop_recording(recording)` | 録音を止め、f32のサンプル列を返す |
| `save_wav(samples, sample_rate, channels, path)` | サンプル列をWAVとして保存する |
| `save_mp3(samples, sample_rate, channels, path)` | サンプル列をMP3として保存する |

```rust
use sss_scene_capture_lib::*;
use std::path::Path;

let image = capture_primary_monitor()?;
save_image(&image, Path::new("screen.png"))?;

let recording = start_recording()?;
let sample_rate = recording.sample_rate;
let channels = recording.channels;
// ...記録中...
let samples = stop_recording(recording);
save_mp3(&samples, sample_rate, channels, Path::new("sound.mp3"))?;
```

録音中は`cpal`のストリームを録音専用スレッドだけが所有する。
そのため`Recording`は`Send`であり、別言語からFFIで呼ぶこともできる。

## Javaなど別言語から使う（FFI）

`cargo build --release`で`target/release/sss_scene_capture_lib.dll`ができる。
このDLLはC ABIの関数を公開している（`src/ffi.rs`、ADR-003、ADR-005）。

| 関数 | 役割 |
| --- | --- |
| `u64 sss_start_recording()` | 録音を始め、IDを返す。0は失敗 |
| `i32 sss_stop_recording(u64 id, i32 format, const char* path)` | IDの録音を止め、`path`に保存する。0は成功 |
| `i32 sss_capture_primary_monitor(const char* path)` | プライマリーモニターをPNGとして`path`に保存する。0は成功 |

`format`は`0`がWAV、`1`がMP3である。`path`はUTF-8のNUL終端文字列である。

戻り値のコードは次の通り。

| コード | 意味 |
| --- | --- |
| 0 | 成功 |
| 1 | IDが無効（未知、または停止済み） |
| 2 | パスがNULL、またはUTF-8でない |
| 3 | 形式コードが不正 |
| 4 | ファイルの保存に失敗 |
| 5 | スクリーンショットの取得に失敗 |
| 99 | Rust側で内部エラー（panic） |

不正な引数を渡したとき、録音は止まらない。引数を直して再度停止できる。

Java 22以降のFFM APIから呼ぶ例。JNIやJNAは不要である。

```java
import java.lang.foreign.*;
import java.lang.invoke.MethodHandle;

public class SceneCapture {
    public static void main(String[] args) throws Throwable {
        Linker linker = Linker.nativeLinker();
        SymbolLookup lib = SymbolLookup.libraryLookup("sss_scene_capture_lib.dll", Arena.global());

        MethodHandle capture = linker.downcallHandle(
            lib.find("sss_capture_primary_monitor").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS));
        MethodHandle start = linker.downcallHandle(
            lib.find("sss_start_recording").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_LONG));
        MethodHandle stop = linker.downcallHandle(
            lib.find("sss_stop_recording").orElseThrow(),
            FunctionDescriptor.of(ValueLayout.JAVA_INT,
                ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT, ValueLayout.ADDRESS));

        try (Arena arena = Arena.ofConfined()) {
            int code = (int) capture.invokeExact(arena.allocateFrom("screen.png"));
            System.out.println("capture: " + code);

            long id = (long) start.invokeExact();
            Thread.sleep(3000);
            code = (int) stop.invokeExact(id, 1, arena.allocateFrom("sound.mp3"));
            System.out.println("stop: " + code);
        }
    }
}
```

```
java --enable-native-access=ALL-UNNAMED SceneCapture.java
```

`Arena.allocateFrom(String)`はUTF-8のNUL終端文字列を確保する。DLLはカレントディレクトリか`java.library.path`に置く。

## 使用クレート

- [xcap](https://crates.io/crates/xcap): スクリーンショット取得
- [cpal](https://crates.io/crates/cpal): WASAPIループバック録音
- [hound](https://crates.io/crates/hound): WAV書き出し
- [rusty_mp3](https://crates.io/crates/rusty_mp3): MP3エンコード
- [chrono](https://crates.io/crates/chrono): 日時名フォルダの生成

## 開発

```
cargo test
```

設計上の判断は`docs/adr/`に記録している。
機能の全体像は`docs/storymap.md`にある。

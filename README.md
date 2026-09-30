# sss-scene-capture-lib

アプリの画面とサウンドを記録するWindows専用のCLIツール、およびそのライブラリ。

1回のセッションにつき、プライマリーモニターを1枚の静止画として保存する。
静止画は開始時に撮る。録音を止めたあとに撮り直すこともできる。
録音の開始から停止までのPCのシステムサウンドをWAVまたはMP3として保存する。

## 動作環境

- Windows 10以降

システムサウンドはWASAPIのループバック録音で取得する。マイクは録音しない。

## 使い方

```
cargo run --release
```

実行ファイルは`target/release/sss-scene-capture.exe`である。

1. `Press Enter to start...`と表示されたらEnterキーを押す。
2. スクリーンショットが撮られ、録音が始まる。
3. もう一度Enterキーを押すと録音が止まる。
4. 次のいずれかを入力する。
   - Enter: ファイルを保存し、次のセッションを始める。
   - `s`+Enter: スクリーンショットを撮り直して置き換える。何度でもできる。
   - `q`+Enter: 保存せずに終了する。
5. Ctrl+Cでもいつでも終了できる。

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
そのため`Recording`は`Send`であり、開始と停止を別のスレッドから呼べる。

## Javaなど別言語から使う（FFI）

`cargo build --release`で`target/release/sss_scene_capture_lib.dll`ができる。
このDLLはC ABIの関数を公開している。関数、戻り値のコード、Java（FFM API）からの呼び出し例は[docs/ffi.md](docs/ffi.md)を見ること。

## ライセンス

[MIT License](LICENSE)

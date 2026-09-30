# Javaなど別言語から使う（FFI）

`cargo build --release`で`target/release/sss_scene_capture_lib.dll`ができる。
このDLLはC ABIの関数を公開している（`src/ffi.rs`）。

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

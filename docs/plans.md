# plans

TDDで現在対象としているタスクの作業用todo。使い捨て。

### テストリスト

- [x] サウンドはMP3に変換して保存することもできる
  - [x] 音声がMP3として指定のファイル名で保存される（save_mp3）。

- [x] s+Enterで、現在のプライマリーモニターのスクリーンショットで置換される

- [x] Javaから呼ぶ（C ABIのFFIを追加する。設計はADR-003、ADR-005）
  - [x] sss_start_recordingは0以外のIDを返す
  - [x] 無効なIDでsss_stop_recordingを呼ぶと、エラーコードを返す
  - [ ] sss_stop_recordingは、形式コードWAVで指定パスにファイルを書き、0を返す
  - [ ] sss_stop_recordingは、形式コードMP3で指定パスにファイルを書き、0を返す
  - [x] 同じIDで2回目のsss_stop_recordingを呼ぶと、エラーコードを返す
  - [x] 不正な形式コードを渡すと、エラーコードを返す（録音は止めない）
  - [x] パスがNULLのとき、エラーコードを返す
  - [x] sss_capture_primary_monitorは、指定パスにPNGを書き、0を返す
  - [x] Cargo.tomlのcrate-typeにcdylibを足し、DLLが生成される
  - [x] READMEにJava（FFM API）からの呼び出し例を書く
  - [x] ADR-005（形式コードとスクリーンショットのFFI）を書く

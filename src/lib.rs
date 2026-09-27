use std::io::BufRead;
use std::io::Write;
use std::time::Duration;

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
}

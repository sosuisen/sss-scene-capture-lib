use std::io::Write;

pub fn show_startup_prompt(writer: &mut impl Write) -> std::io::Result<()> {
    writeln!(writer, "Press Enter to start...")?;
    Ok(())
}

#[cfg(test)]
mod tests {
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
}

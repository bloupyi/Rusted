//! Styled custom names, as tooltip/held-item text draws them.
use horizon_client::{item_data::parse_stack, item_tooltip, text::Style};
use std::{error::Error, fmt::Write};
pub fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = crate::args().collect();
    let input = std::fs::read_to_string(&args[1])?;
    let mut output = String::new();
    for line in input.lines().filter(|line| !line.is_empty()) {
        let stack = parse_stack(line)?.ok_or("empty fixture stack")?;
        let text = item_tooltip::name_text(&stack, &|_| None).with_style(&Style::color(0xffffff));
        let spans = text
            .spans
            .iter()
            .filter(|s| !s.text.is_empty())
            .map(|s| {
                format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    s.text,
                    s.style.color.map(|c| c as i32).unwrap_or(-1),
                    s.style.is_bold(),
                    s.style.is_italic(),
                    s.style.underlined == Some(true),
                    s.style.strikethrough == Some(true),
                    s.style.obfuscated == Some(true)
                )
            })
            .collect::<Vec<_>>();
        writeln!(output, "{}", spans.join(" | "))?;
    }
    std::fs::write(&args[2], output)?;
    Ok(())
}

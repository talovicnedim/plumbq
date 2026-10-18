//! Text from an agent or an imported file as the approval dialog shows it. Line breaks could
//! fake lines of the dialog, and Unicode format characters (bidi overrides, zero-width
//! characters) could reorder or hide what the user reads, so both are shown escaped.

/// Longer text from outside is cut, with a note.
const MAX_SHOWN_CHARS: usize = 300;
/// The AI app's name, as it introduced itself, starts the dialog's title.
const MAX_NAME_CHARS: usize = 60;

/// One value on one line, cut at [`MAX_SHOWN_CHARS`].
pub(crate) fn shown(text: &str) -> String {
    cut_escaped(text, MAX_SHOWN_CHARS)
}

/// A name on one line, cut at [`MAX_NAME_CHARS`].
pub(super) fn shown_name(text: &str) -> String {
    cut_escaped(text, MAX_NAME_CHARS)
}

fn cut_escaped(text: &str, max: usize) -> String {
    let mut out = escaped(&text.chars().take(max).collect::<String>());
    if text.chars().nth(max).is_some() {
        out.push_str("... (cut)");
    }
    out
}

/// A whole sentence on one line (actions, warnings), not cut.
pub(super) fn escaped(text: &str) -> String {
    text.chars().map(escape).collect()
}

/// A multi-line detail (a request, a diff): its lines stay lines, everything else is escaped.
pub(super) fn escaped_lines(text: &str) -> String {
    text.replace("\r\n", "\n")
        .split('\n')
        .map(|line| {
            line.chars()
                .map(|c| if c == '\t' { c.to_string() } else { escape(c) })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn escape(c: char) -> String {
    match c {
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        c if c.is_control() || is_format(c) => format!("\\u{{{:x}}}", u32::from(c)),
        c => c.to_string(),
    }
}

/// Unicode format (Cf) and line or paragraph separator (Zl, Zp) characters that change how
/// text around them is shown.
fn is_format(c: char) -> bool {
    matches!(
        c,
        '\u{ad}'
            | '\u{61c}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{2028}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
    )
}

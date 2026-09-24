/// A transfer rate, nine characters at most.
///
/// The idle capsule reserves a fixed cell for this string, so a tenth character
/// would push the figure past the gap that follows its reading. The scale tops
/// out wherever the string would otherwise grow: `B/s` stops at `1023 B/s`,
/// `KB/s` at `1023 KB/s`, `MB/s` drops its decimal at 100 (`999 MB/s`, never
/// `999.9 MB/s`) and `GB/s` does the same at 100 GB/s.
pub fn format_rate(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let value = bytes as f64;
    if value >= GB {
        format!("{} GB/s", figure(value / GB))
    } else if value >= MB {
        format!("{} MB/s", figure(value / MB))
    } else if value >= KB {
        format!("{:.0} KB/s", value / KB)
    } else {
        format!("{bytes} B/s")
    }
}

/// A rate's figure: one decimal below a hundred, and never five characters.
///
/// The threshold is at 99.95 rather than 100 so that `99.99` rounds to the
/// three-character `100` instead of the four-character `100.0`.
fn figure(value: f64) -> String {
    if value >= 99.95 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let value = bytes as f64;
    if value >= GB {
        format!("{:.2} GB", value / GB)
    } else if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.0} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_changes_unit_without_changing_scale() {
        assert_eq!(format_rate(0), "0 B/s");
        assert_eq!(format_rate(1023), "1023 B/s");
        assert_eq!(format_rate(1024), "1 KB/s");
        assert_eq!(format_rate(1024 * 1023), "1023 KB/s");
        assert_eq!(format_rate(1024 * 1024), "1.0 MB/s");
        assert_eq!(format_rate(100 * 1024 * 1024), "100 MB/s");
        assert_eq!(format_rate(1024 * 1024 * 1024), "1.0 GB/s");
    }

    /// The rate cell in the idle strip is nine characters wide, and the whole
    /// point of that cell is that a figure growing longer cannot reach the icon
    /// after it. Anything wider than nine would.
    #[test]
    fn rate_never_grows_past_nine_characters() {
        let mut bytes = 0u64;
        while bytes < 4096 {
            check_rate_width(bytes);
            bytes += 1;
        }
        // Half-megabyte steps across every unit change, including the values
        // that round up over a unit boundary.
        let mut halves = 0u64;
        while halves <= 3000 {
            check_rate_width(halves * 512 * 1024);
            halves += 1;
        }
        let mut mib = 100 * 1024;
        while mib <= 102 * 1024 {
            check_rate_width(mib * 1024 * 1024);
            mib += 1;
        }
    }

    fn check_rate_width(bytes: u64) {
        let text = format_rate(bytes);
        assert!(text.chars().count() <= 9, "{bytes} renders as {text}");
    }
}

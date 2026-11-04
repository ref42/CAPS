#[derive(Clone, Debug, PartialEq)]
pub struct LyricLine {
    time: f64,
    text: String,
}

pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    // LRC offsets are milliseconds; positive values advance the lyrics.
    // Metadata can appear after the timed lines, so apply it after parsing.
    let mut offset = 0.0;
    for raw in text.lines() {
        let mut rest = raw.trim().trim_start_matches('\u{feff}');
        let mut times = Vec::new();
        while let Some(after_open) = rest.strip_prefix('[') {
            let Some((stamp, after_stamp)) = after_open.split_once(']') else {
                break;
            };
            if let Some(time) = parse_lrc_time(stamp) {
                times.push(time);
            } else if let Some(value) = stamp.strip_prefix("offset:")
                && let Ok(milliseconds) = value.trim().parse::<i64>()
            {
                offset = milliseconds as f64 / 1000.;
            }
            rest = after_stamp.trim_start();
        }
        let lyric = rest.trim();
        // Empty timed lines end the previous lyric during instrumental breaks.
        for time in times {
            lines.push(LyricLine {
                time,
                text: lyric.to_string(),
            });
        }
    }
    for line in &mut lines {
        line.time -= offset;
    }
    lines.sort_by(|a, b| a.time.total_cmp(&b.time));
    lines
}

pub fn current_lyric_line(lines: &[LyricLine], position: f64) -> Option<String> {
    let index = lines.partition_point(|line| line.time <= position);
    index
        .checked_sub(1)
        .and_then(|index| lines.get(index))
        .map(|line| line.text.to_owned())
}

fn parse_lrc_time(text: &str) -> Option<f64> {
    let (minutes, seconds) = text.split_once(':')?;
    let minutes = minutes.parse::<f64>().ok()?;
    let seconds = seconds.parse::<f64>().ok()?;
    if !minutes.is_finite()
        || !seconds.is_finite()
        || minutes < 0.
        || minutes.fract() != 0.
        || !(0.0..60.0).contains(&seconds)
    {
        return None;
    }
    Some(minutes * 60.0 + seconds)
}

#[cfg(test)]
mod tests {
    use super::{current_lyric_line, parse_lrc};

    #[test]
    fn parses_provider_lyrics_and_selects_current_line() {
        let lines = parse_lrc("[ti:Song]\n[00:00.00]Intro\n[00:01.50]Verse");
        assert_eq!(lines.len(), 2);
        assert_eq!(current_lyric_line(&lines, 1.45).as_deref(), Some("Intro"));
        assert_eq!(current_lyric_line(&lines, 1.5).as_deref(), Some("Verse"));
    }

    #[test]
    fn honors_offsets_anywhere_in_file_and_handles_backward_seeks() {
        let lines = parse_lrc("\u{feff}[00:02.00]First\n[00:05.00]Second\n[offset:1500]");
        assert_eq!(current_lyric_line(&lines, 0.5).as_deref(), Some("First"));
        assert_eq!(current_lyric_line(&lines, 3.5).as_deref(), Some("Second"));
        assert_eq!(current_lyric_line(&lines, 1.).as_deref(), Some("First"));
        let delayed = parse_lrc("[offset:-500]\n[00:01.00]Delayed");
        assert_eq!(current_lyric_line(&delayed, 1.49), None);
        assert_eq!(
            current_lyric_line(&delayed, 1.5).as_deref(),
            Some("Delayed")
        );
    }

    #[test]
    fn clears_instrumental_gaps_and_rejects_invalid_timestamps() {
        let lines = parse_lrc(
            "[00:01]Verse\n[00:03]\n[NaN:01]Bad\n[00:inf]Bad\n[00:60]Bad\n[00:05][00:07]Chorus",
        );
        assert_eq!(lines.len(), 4);
        assert_eq!(current_lyric_line(&lines, 4.).as_deref(), Some(""));
        assert_eq!(current_lyric_line(&lines, 7.).as_deref(), Some("Chorus"));
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LyricLine {
    time: f64,
    text: String,
}

pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(after_open) = rest.strip_prefix('[') {
            let Some((stamp, after_stamp)) = after_open.split_once(']') else {
                break;
            };
            if let Some(time) = parse_lrc_time(stamp) {
                times.push(time);
            }
            rest = after_stamp.trim_start();
        }
        let lyric = rest.trim();
        if lyric.is_empty() {
            continue;
        }
        for time in times {
            lines.push(LyricLine {
                time,
                text: lyric.to_string(),
            });
        }
    }
    lines.sort_by(|a, b| a.time.total_cmp(&b.time));
    lines
}

pub fn current_lyric_line(lines: &[LyricLine], position: f64) -> Option<String> {
    let target = position + 0.55;
    lines
        .iter()
        .take_while(|line| line.time <= target)
        .last()
        .map(|line| line.text.clone())
}

fn parse_lrc_time(text: &str) -> Option<f64> {
    let (minutes, seconds) = text.split_once(':')?;
    let minutes = minutes.parse::<f64>().ok()?;
    let seconds = seconds.parse::<f64>().ok()?;
    Some(minutes * 60.0 + seconds)
}

#[cfg(test)]
mod tests {
    use super::{current_lyric_line, parse_lrc};

    #[test]
    fn parses_provider_lyrics_and_selects_current_line() {
        let lines = parse_lrc("[ti:Song]\n[00:00.00]Intro\n[00:01.50]Verse");
        assert_eq!(lines.len(), 2);
        assert_eq!(current_lyric_line(&lines, 1.2).as_deref(), Some("Verse"));
    }
}

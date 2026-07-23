use std::path::Path;

#[derive(Debug, Clone)]
pub struct LyricLine {
    pub time_secs: f64,
    pub text: String,
}

pub fn parse(content: &str) -> Vec<LyricLine> {
    let mut lines: Vec<LyricLine> = Vec::new();

    for raw in content.lines() {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }

        
        let mut pos = 0;
        let mut timestamps: Vec<f64> = Vec::new();

        while pos < raw.len() && raw[pos..].starts_with('[') {
            if let Some(close) = raw[pos..].find(']') {
                let tag = &raw[pos + 1..pos + close];
                if let Some(t) = parse_timestamp(tag) {
                    timestamps.push(t);
                    pos += close + 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if timestamps.is_empty() {
            continue;
        }

        let text = raw[pos..].trim().to_owned();
        for t in timestamps {
            lines.push(LyricLine {
                time_secs: t,
                text: text.clone(),
            });
        }
    }

    lines.sort_by(|a, b| a.time_secs.partial_cmp(&b.time_secs).unwrap());
    lines
}

fn parse_timestamp(s: &str) -> Option<f64> {
    
    let colon = s.find(':')?;
    let mins: f64 = s[..colon].trim().parse().ok()?;
    let secs: f64 = s[colon + 1..].trim().parse().ok()?;
    Some(mins * 60.0 + secs)
}


pub fn current_line(lines: &[LyricLine], seconds: f64) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }
    let idx = lines.partition_point(|l| l.time_secs <= seconds);
    if idx == 0 {
        None
    } else {
        Some(idx - 1)
    }
}


pub fn from_sidecar(track_path: &Path) -> Option<Vec<LyricLine>> {
    let lrc = track_path.with_extension("lrc");
    let content = std::fs::read_to_string(&lrc).ok()?;
    let lines = parse(&content);
    if lines.is_empty() {
        None
    } else {
        Some(lines)
    }
}

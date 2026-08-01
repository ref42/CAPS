use std::path::Path;

use tokio::io::AsyncWriteExt;

const PARALLEL_THRESHOLD: u64 = 8 * 1024 * 1024;
const RANGE_CHUNK_SIZE: u64 = 8 * 1024 * 1024;
const MAX_PARALLEL_RANGES: usize = 6;

#[derive(Clone, Copy, Debug)]
struct RangePart {
    index: usize,
    start: u64,
    end: u64,
}

pub async fn download_url_to_path_with_progress<F>(
    client: &reqwest::Client,
    url: &str,
    referer: &str,
    expected_size: Option<u64>,
    temp_path: &Path,
    label: &str,
    empty_message: &str,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64, Option<u64>),
{
    let _ = tokio::fs::remove_file(temp_path).await;

    if let Some(total) = probe_range_total(client, url, referer).await {
        if total >= PARALLEL_THRESHOLD
            && parallel_range_download(
                client,
                url,
                referer,
                total,
                temp_path,
                label,
                &mut on_progress,
            )
            .await
            .is_ok()
        {
            return Ok(());
        }
        let _ = tokio::fs::remove_file(temp_path).await;
    }

    serial_download(
        client,
        url,
        referer,
        expected_size,
        temp_path,
        label,
        empty_message,
        on_progress,
    )
    .await
}

async fn serial_download<F>(
    client: &reqwest::Client,
    url: &str,
    referer: &str,
    expected_size: Option<u64>,
    temp_path: &Path,
    label: &str,
    empty_message: &str,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64, Option<u64>),
{
    let mut response = client
        .get(url)
        .header(reqwest::header::REFERER, referer)
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .send()
        .await
        .map_err(|err| format!("{label} request failed: {err}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "{label} request failed: HTTP {}",
            response.status()
        ));
    }

    let total = response.content_length().or(expected_size);
    let mut file = tokio::fs::File::create(temp_path)
        .await
        .map_err(|err| format!("Audio cache write failed: {err}"))?;
    let mut written = 0_u64;
    on_progress(written, total);
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|err| format!("{label} read failed: {err}"))?
    {
        written += chunk.len() as u64;
        file.write_all(&chunk)
            .await
            .map_err(|err| format!("Audio cache write failed: {err}"))?;
        on_progress(written, total);
    }
    file.flush()
        .await
        .map_err(|err| format!("Audio cache write failed: {err}"))?;

    if written == 0 {
        return Err(empty_message.to_string());
    }
    Ok(())
}

async fn parallel_range_download<F>(
    client: &reqwest::Client,
    url: &str,
    referer: &str,
    total: u64,
    temp_path: &Path,
    label: &str,
    on_progress: &mut F,
) -> Result<(), String>
where
    F: FnMut(u64, Option<u64>),
{
    let parts = range_parts(total);
    let mut file = tokio::fs::File::create(temp_path)
        .await
        .map_err(|err| format!("Audio cache write failed: {err}"))?;
    let mut downloaded = 0_u64;
    on_progress(downloaded, Some(total));

    for wave in parts.chunks(MAX_PARALLEL_RANGES) {
        let mut tasks = Vec::with_capacity(wave.len());
        for part in wave.iter().copied() {
            let client = client.clone();
            let url = url.to_string();
            let referer = referer.to_string();
            let label = label.to_string();
            tasks.push(tokio::spawn(async move {
                download_range_part(client, url, referer, part, label).await
            }));
        }

        let mut results = Vec::with_capacity(wave.len());
        for task in tasks {
            let result = task
                .await
                .map_err(|err| format!("{label} range task failed: {err}"))??;
            downloaded += result.1.len() as u64;
            on_progress(downloaded.min(total), Some(total));
            results.push(result);
        }
        results.sort_by_key(|(part, _)| part.index);
        for (_, bytes) in results {
            file.write_all(&bytes)
                .await
                .map_err(|err| format!("Audio cache write failed: {err}"))?;
        }
    }

    file.flush()
        .await
        .map_err(|err| format!("Audio cache write failed: {err}"))?;
    Ok(())
}

async fn download_range_part(
    client: reqwest::Client,
    url: String,
    referer: String,
    part: RangePart,
    label: String,
) -> Result<(RangePart, Vec<u8>), String> {
    let mut response = client
        .get(url)
        .header(reqwest::header::REFERER, referer)
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .header(
            reqwest::header::RANGE,
            format!("bytes={}-{}", part.start, part.end),
        )
        .send()
        .await
        .map_err(|err| format!("{label} range request failed: {err}"))?;

    if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(format!(
            "{label} range request failed: HTTP {}",
            response.status()
        ));
    }

    let expected_len = part.end - part.start + 1;
    let mut bytes = Vec::with_capacity(expected_len.min(usize::MAX as u64) as usize);
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|err| format!("{label} range read failed: {err}"))?
    {
        bytes.extend_from_slice(&chunk);
    }
    if bytes.len() as u64 != expected_len {
        return Err(format!(
            "{label} range returned {} bytes for {} bytes requested",
            bytes.len(),
            expected_len
        ));
    }
    Ok((part, bytes))
}

async fn probe_range_total(client: &reqwest::Client, url: &str, referer: &str) -> Option<u64> {
    let response = client
        .get(url)
        .header(reqwest::header::REFERER, referer)
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .header(reqwest::header::RANGE, "bytes=0-0")
        .send()
        .await
        .ok()?;

    if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return None;
    }

    response
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_content_range_total)
}

fn range_parts(total: u64) -> Vec<RangePart> {
    let mut parts = Vec::new();
    let mut start = 0_u64;
    while start < total {
        let end = (start + RANGE_CHUNK_SIZE - 1).min(total - 1);
        parts.push(RangePart {
            index: parts.len(),
            start,
            end,
        });
        start = end + 1;
    }
    parts
}

fn parse_content_range_total(value: &str) -> Option<u64> {
    let (_, total) = value.rsplit_once('/')?;
    total.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_content_range_total() {
        assert_eq!(parse_content_range_total("bytes 0-0/12345"), Some(12345));
        assert_eq!(parse_content_range_total("bytes 0-0/*"), None);
    }

    #[test]
    fn splits_range_parts() {
        let parts = range_parts(RANGE_CHUNK_SIZE + 7);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].start, 0);
        assert_eq!(parts[0].end, RANGE_CHUNK_SIZE - 1);
        assert_eq!(parts[1].start, RANGE_CHUNK_SIZE);
        assert_eq!(parts[1].end, RANGE_CHUNK_SIZE + 6);
    }
}

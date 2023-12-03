use std::{cmp, fs::File, io::Write};

use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};

// https://gist.github.com/giuliano-oliveira/4d11d6b3bb003dba3a1b53f43d81b30d
pub async fn download_file(client: &reqwest::Client, url: &str, path: &str) -> anyhow::Result<()> {
    // Reqwest setup
    let res = client
        .get(url)
        .send()
        .await
        .or(Err(anyhow::anyhow!("Failed to GET from '{url}'")))?;
    let total_size = res
        .content_length()
        .ok_or(anyhow::anyhow!("Failed to get content length from '{url}'"))?;

    // Indicatif setup
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")?
        .progress_chars("#>-"));
    pb.set_message(format!("Downloading {url}"));

    // download chunks
    let mut file = File::create(path).or(Err(anyhow::anyhow!("Failed to create file '{path}'")))?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.or(Err(anyhow::anyhow!("Error while downloading file")))?;
        file.write_all(&chunk)
            .or(Err(anyhow::anyhow!("Error while writing to file")))?;
        let new = cmp::min(downloaded + (chunk.len() as u64), total_size);
        downloaded = new;
        pb.set_position(new);
    }

    pb.finish_with_message(format!("Downloaded {url} to {path}"));
    Ok(())
}

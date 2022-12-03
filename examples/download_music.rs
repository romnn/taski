#![allow(warnings)]

// use taski::*;
use anyhow::Result;
use async_process::Command;
use futures::stream::StreamExt;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

// struct Entry {
//     // name: String,
//     start: Instant,
//     end: Instant,
// }

struct DebugTrace {
    start: Arc<Mutex<HashMap<String, Instant>>>,
    end: Arc<Mutex<HashMap<String, Instant>>>,
}

impl DebugTrace {
    pub async fn trace_start(&self, name: impl Into<String>) {
        self.start.lock().await.insert(name.into(), Instant::now());
    }

    pub async fn trace_end(&self, name: impl Into<String>) {
        self.end.lock().await.insert(name.into(), Instant::now());
    }
}

#[derive(Debug)]
struct ExtractAudio {
    // cannot hash a TempDir !!!
    tmp_dir: TempDir,
    // file: reqwest::Url,
}

impl ExtractAudio {
    pub fn new() -> Result<Self> {
        let tmp_dir = TempDir::new()?;
        Ok(Self { tmp_dir })
    }
}

impl ExtractAudio {
    pub async fn run(&mut self, input: impl AsRef<Path>) -> Result<PathBuf> {
        // "ffmpeg -i sample.avi -q:a 0 -map a sample.mp3"
        let audio_file = self
            .tmp_dir
            .path()
            .join(input.as_ref().file_name().unwrap())
            .with_extension("mp3");
        let output = Command::new("ffmpeg")
            .args([
                "-i",
                &input.as_ref().to_string_lossy(),
                "-q:a",
                "0",
                "-map",
                "a",
                &audio_file.to_string_lossy(),
            ])
            .output()
            .await?;
        if !output.status.success() {
            anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(audio_file)
    }
}

#[derive(Debug)]
struct Download {
    tmp_dir: TempDir,
    url: reqwest::Url,
}

impl Download {
    pub fn new(url: impl AsRef<str>) -> Result<Self> {
        let url = reqwest::Url::parse(url.as_ref())?;
        let tmp_dir = TempDir::new()?;
        Ok(Self { url, tmp_dir })
    }
}

impl Download {
    pub async fn run(&mut self) -> Result<PathBuf> {
        let client = reqwest::Client::new();
        let res = client.get(self.url.clone()).send().await?;
        let path = self.tmp_dir.path().join("download");
        let mut file = tokio::fs::File::create(&path).await?;
        let mut stream = res.bytes_stream();

        while let Some(chunk) = stream.next().await {
            file.write_all(&chunk?).await?;
        }
        Ok(path)
    }
}

// println!("{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));

#[tokio::main]
async fn main() -> Result<()> {
    let mut extract = ExtractAudio::new()?;
    let mut dl = Download::new("https://download.samplelib.com/mp4/sample-15s.mp4")?;

    // download file
    let video_path = dl.run().await?;
    dbg!(&video_path);

    // extract audio
    let audio_path = extract.run(video_path).await?;
    dbg!(&audio_path);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_temp_dir_hashing() -> Result<()> {
        use std::hash::Hash;
        let e1 = ExtractAudio::new()?;
        let e2 = ExtractAudio::new()?;
        assert_eq!((&e1).hash(), (&e2).hash());
        Ok(())
    }
}

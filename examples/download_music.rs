#![allow(warnings)]

use taski::*;
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
struct CombineAudio {
    // cannot hash a TempDir !!!
    tmp_dir: TempDir,
    // file: reqwest::Url,
}

impl CombineAudio {
    pub fn new() -> Result<Self> {
        let tmp_dir = TempDir::new()?;
        Ok(Self { tmp_dir })
    }
}

impl CombineAudio {
    pub async fn run(
        &mut self,
        audio1: impl AsRef<Path>,
        audio2: impl AsRef<Path>,
    ) -> Result<PathBuf> {
        // "ffmpeg -i sample.avi -q:a 0 -map a sample.mp3"
        // ffmpeg -i "concat:file1.mp3|file2.mp3" -acodec copy output.mp3
        let audio1 = audio1.as_ref();
        let audio2 = audio2.as_ref();
        let audio_file = self
            .tmp_dir
            .path()
            .join(format!(
                "{} + {}",
                audio1
                    .file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .map(urlencoding::decode)
                    .map(Result::ok)
                    .flatten()
                    .unwrap(),
                audio2
                    .file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .map(urlencoding::decode)
                    .map(Result::ok)
                    .flatten()
                    .unwrap(),
            ))
            .with_extension("mp3");
        let output = Command::new("ffmpeg")
            .args([
                "-i",
                &format!("concat:{}|{}", &audio1.display(), &audio2.display()),
                "-acodec",
                "copy",
                // "-q:a",
                // "0",
                // "-map",
                // "a",
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
        let filename = self
            .url
            .path_segments()
            .and_then(|s| s.last())
            .unwrap_or("download");
        let path = self.tmp_dir.path().join(filename);
        let mut file = tokio::fs::File::create(&path).await?;
        let mut stream = res.bytes_stream();

        while let Some(chunk) = stream.next().await {
            file.write_all(&chunk?).await?;
        }
        Ok(path)
    }
}

// println!("{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));

static INCOMPETECH: &str = "https://incompetech.com/music/royalty-free/mp3-royaltyfree";

#[tokio::main]
async fn main() -> Result<()> {
//     let mut combine = CombineAudio::new()?;
//     // let mut dl = Download::new("https://download.samplelib.com/mp4/sample-15s.mp4")?;
//     let mut dl1 =
//         Download::new(INCOMPETECH.to_owned() + "/I%20Got%20a%20Stick%20Arr%20Bryan%20Teoh.mp3")?;

//     let mut dl2 = Download::new(INCOMPETECH.to_owned() + "/The%20Ice%20Giants.mp3")?;

//     // download file
//     let audio1_path = dl1.run().await?;
//     dbg!(&audio1_path);

//     let audio2_path = dl2.run().await?;
//     dbg!(&audio2_path);

//     // extract audio
//     let audio_path = combine.run(audio1_path, audio2_path).await?;
//     dbg!(&audio_path);

//     // copy to example dir so we can test
//     let output_path = PathBuf::from(file!()).with_extension("mp3");
//     tokio::fs::copy(audio_path, output_path).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    // async fn test_temp_dir_hashing() -> Result<()> {
    //     use std::hash::Hash;
    //     let e1 = CombineAudio::new()?;
    //     let e2 = CombineAudio::new()?;
    //     assert_eq!((&e1).hash(), (&e2).hash());
    //     Ok(())
    // }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_concurrent() -> Result<()> {
        let mut combine = CombineAudio::new()?;
        let mut dl1 = Download::new(
            INCOMPETECH.to_owned() + "/I%20Got%20a%20Stick%20Arr%20Bryan%20Teoh.mp3",
        )?;

        let mut dl2 = Download::new(INCOMPETECH.to_owned() + "/The%20Ice%20Giants.mp3")?;

        // download files
        let audio1_path = dl1.run().await?;
        dbg!(&audio1_path);

        let audio2_path = dl2.run().await?;
        dbg!(&audio2_path);

        // combine audio files
        let audio_path = combine.run(audio1_path, audio2_path).await?;
        dbg!(&audio_path);

        // copy to example dir so we can listen
        let output_path = PathBuf::from(file!()).with_extension("mp3");
        tokio::fs::copy(audio_path, output_path).await?;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_serial() -> Result<()> {
        let mut combine = CombineAudio::new()?;
        let mut dl1 = Download::new(
            INCOMPETECH.to_owned() + "/I%20Got%20a%20Stick%20Arr%20Bryan%20Teoh.mp3",
        )?;

        let mut dl2 = Download::new(INCOMPETECH.to_owned() + "/The%20Ice%20Giants.mp3")?;

        // download files
        let audio1_path = dl1.run().await?;
        dbg!(&audio1_path);

        let audio2_path = dl2.run().await?;
        dbg!(&audio2_path);

        // combine audio files
        let audio_path = combine.run(audio1_path, audio2_path).await?;
        dbg!(&audio_path);

        // copy to example dir so we can listen
        let output_path = PathBuf::from(file!()).with_extension("mp3");
        tokio::fs::copy(audio_path, output_path).await?;

        Ok(())
    }
}

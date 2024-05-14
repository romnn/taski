mod concurrent;
mod serial;

use async_process::Command;
use clap::Parser;
use color_eyre::eyre;
use futures::stream::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;

static INCOMPETECH: &str = "https://incompetech.com/music/royalty-free/mp3-royaltyfree";

#[derive(Debug, Copy, Clone)]
enum Label {
    Input,
    Download,
    Combine,
}

#[derive(Clone, Debug)]
struct Download {
    tmp_dir: Arc<TempDir>,
    client: reqwest::Client,
}

impl std::fmt::Display for Download {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Download")
            .field("tmp_dir", &self.tmp_dir.path())
            .finish()
    }
}

#[async_trait::async_trait]
impl taski::Task1<(reqwest::Url, String), PathBuf> for Download {
    async fn run(self: Box<Self>, input: (reqwest::Url, String)) -> taski::TaskResult<PathBuf> {
        let (url, file_name) = input;
        let dest_path = self.tmp_dir.path().join(file_name);

        println!("downloading {} to {}", url, dest_path.display());

        let res = self.client.get(url).send().await?;
        let mut dest_file = tokio::fs::File::create(&dest_path).await?;
        let mut stream = res.bytes_stream();

        while let Some(chunk) = stream.next().await {
            dest_file.write_all(&chunk?).await?;
        }
        Ok(dest_path)
    }

    fn name(&self) -> String {
        format!("{self}")
    }
}

#[derive(Debug, Clone)]
struct CombineAudio {
    tmp_dir: Arc<TempDir>,
}

impl std::fmt::Display for CombineAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombineAudio")
            .field("tmp_dir", &self.tmp_dir.path())
            .finish()
    }
}

#[async_trait::async_trait]
impl taski::Task2<PathBuf, PathBuf, PathBuf> for CombineAudio {
    async fn run(
        self: Box<Self>,
        audio1_path: PathBuf,
        audio2_path: PathBuf,
    ) -> taski::TaskResult<PathBuf> {
        // "ffmpeg -i sample.avi -q:a 0 -map a sample.mp3"
        // ffmpeg -i "concat:file1.mp3|file2.mp3" -acodec copy output.mp3
        let output_path = self.tmp_dir.path().join("out.mp3");

        println!(
            "combining {} and {}",
            &audio1_path.display(),
            &audio2_path.display()
        );
        let mut cmd = Command::new("ffmpeg");
        cmd.args([
            "-i",
            &format!(
                "concat:{}|{}",
                &audio1_path.display(),
                &audio2_path.display()
            ),
            "-acodec",
            "copy",
            &output_path.to_string_lossy(),
        ]);
        let output = cmd.output().await?;
        if output.status.success() {
            Ok(output_path)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(eyre::eyre!("{}", stderr).into())
        }
    }

    fn name(&self) -> String {
        format!("{self}")
    }
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(std::env!("CARGO_MANIFEST_DIR"))
}

fn should_render() -> bool {
    std::env::var("RENDER")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
        == "yes"
}

#[derive(Debug, Clone, Parser)]
struct Args {
    #[clap(long = "url", help = "URL to a valid audio file")]
    urls: Vec<reqwest::Url>,
    #[clap(short = 'o', long = "output", help = "output path")]
    output_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct Options {
    first_url: reqwest::Url,
    second_url: reqwest::Url,
    output_path: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args = Args::parse();
    let default_urls = [
        reqwest::Url::parse(&format!(
            "{INCOMPETECH}/I%20Got%20a%20Stick%20Arr%20Bryan%20Teoh.mp3"
        ))?,
        reqwest::Url::parse(&format!("{INCOMPETECH}/The%20Ice%20Giants.mp3"))?,
    ];
    let mut urls = args.urls.into_iter().chain(default_urls);
    let first_url = urls.next().unwrap();
    let second_url = urls.next().unwrap();

    let options = Options {
        first_url,
        second_url,
        output_path: args.output_path,
    };
    concurrent::run(options.clone()).await?;
    serial::run(options).await?;
    Ok(())
}

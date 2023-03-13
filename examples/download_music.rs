#![allow(warnings)]

use anyhow::Result;
use async_process::Command;
use async_trait::async_trait;
use futures::stream::StreamExt;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use taski::Dependency;
use taski::*;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

static INCOMPETECH: &str = "https://incompetech.com/music/royalty-free/mp3-royaltyfree";

#[derive(Clone)]
struct CombineAudio {
    tmp_dir: Arc<TempDir>,
}

impl CombineAudio {
    pub fn new(tmp_dir: Arc<TempDir>) -> Self {
        Self { tmp_dir }
    }
}

impl std::fmt::Display for CombineAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombineAudio").finish()
    }
}

impl std::fmt::Debug for CombineAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombineAudio").finish()
    }
}

#[async_trait]
impl taski::Task2<PathBuf, PathBuf, PathBuf> for CombineAudio {
    async fn run(self: Box<Self>, audio1: PathBuf, audio2: PathBuf) -> taski::TaskResult<PathBuf> {
        use std::ffi::OsStr;

        // "ffmpeg -i sample.avi -q:a 0 -map a sample.mp3"
        // ffmpeg -i "concat:file1.mp3|file2.mp3" -acodec copy output.mp3
        let audio_file = self
            .tmp_dir
            .path()
            .join(format!(
                "{} + {}",
                audio1.file_name().and_then(OsStr::to_str).unwrap(),
                audio2.file_name().and_then(OsStr::to_str).unwrap(),
            ))
            .with_extension("mp3");
        let output = Command::new("ffmpeg")
            .args([
                "-i",
                &format!("concat:{}|{}", &audio1.display(), &audio2.display()),
                "-acodec",
                "copy",
                &audio_file.to_string_lossy(),
            ])
            .output()
            .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("{}", stderr).into());
        }
        Ok(audio_file)
    }
}

#[derive(Clone)]
struct Download {
    tmp_dir: Arc<TempDir>,
    client: reqwest::Client,
}

impl std::fmt::Display for Download {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Download").finish()
    }
}

impl std::fmt::Debug for Download {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Download").finish()
    }
}

impl Download {
    pub fn new(tmp_dir: Arc<TempDir>) -> Self {
        let client = reqwest::Client::new();
        Self { tmp_dir, client }
    }
}

fn filename_from_url(url: &reqwest::Url) -> Result<String> {
    let filename = url
        .path_segments()
        .and_then(std::iter::Iterator::last)
        .ok_or(anyhow::anyhow!("failed to get last segement of path {url}"))?;
    let filename = urlencoding::decode(filename)?;
    Ok(filename.to_string())
}

#[async_trait]
impl taski::Task1<String, PathBuf> for Download {
    async fn run(self: Box<Self>, url: String) -> taski::TaskResult<PathBuf> {
        let url = reqwest::Url::parse(url.as_ref())?;
        let filename = filename_from_url(&url)?;
        let res = self.client.get(url).send().await?;
        let path = self.tmp_dir.path().join(filename);
        let mut file = tokio::fs::File::create(&path).await?;
        let mut stream = res.bytes_stream();

        while let Some(chunk) = stream.next().await {
            file.write_all(&chunk?).await?;
        }
        Ok(path)
    }
}

fn with_stem(path: impl AsRef<Path>, func: impl FnOnce(&str) -> String) -> PathBuf {
    let stem = path
        .as_ref()
        .file_stem()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap();
    let extension = path.as_ref().extension().unwrap();
    let new_stem = func(stem);
    path.as_ref()
        .with_file_name(new_stem)
        .with_extension(extension)
}

#[tokio::main]
async fn main() -> Result<()> {
    let start = Instant::now();
    let tmp_dir = Arc::new(TempDir::new()?);

    let download = Download::new(tmp_dir.clone());
    let combine = CombineAudio::new(tmp_dir.clone());

    let mut graph = taski::Schedule::default();
    let audio1_url = format!("{INCOMPETECH}/I%20Got%20a%20Stick%20Arr%20Bryan%20Teoh.mp3");
    let audio2_url = format!("{INCOMPETECH}/The%20Ice%20Giants.mp3");

    let audio1_input = graph.add_node(taski::TaskInput::from(audio1_url), ());
    let audio2_input = graph.add_node(taski::TaskInput::from(audio2_url), ());

    let audio1_download = graph.add_node(download.clone(), (audio1_input,));
    let audio2_download = graph.add_node(download.clone(), (audio2_input,));

    let result_node = graph.add_node(combine, (audio1_download, audio2_download));
    dbg!(&graph);

    let source_file = PathBuf::from(file!());
    let graph_file =
        with_stem(source_file.clone(), |stem| format!("{stem}_dag")).with_extension("svg");
    graph.render_to(graph_file)?;

    // run all tasks
    graph.run().await;
    // debug the graph now
    dbg!(&graph);

    // render trace
    let trace_file =
        with_stem(source_file.clone(), |stem| format!("{stem}_trace")).with_extension("svg");
    graph.render_trace(trace_file).await;

    // // copy to example dir so we can test
    let output_path = source_file.with_extension("mp3");
    tokio::fs::copy(result_node.output().unwrap(), output_path).await?;

    println!("done after: {:?}", start.elapsed());
    Ok(())
}

async fn serial() -> Result<()> {
    // use taski::Task;
    // // let mut combine = CombineAudio::new()?;
    // // // let mut dl = Download::new("https://download.samplelib.com/mp4/sample-15s.mp4")?;
    // let download = Box::new(Download::new()?);
    // // let mut dl1 = Download::new(format!(
    // //     "{INCOMPETECH}/I%20Got%20a%20Stick%20Arr%20Bryan%20Teoh.mp3"
    // // ))?;

    // // let mut dl2 = Download::new(format!("{INCOMPETECH}/The%20Ice%20Giants.mp3"))?;

    // // download files
    // let audio1_path = taski::Task1::run(
    //     download,
    //     format!("{INCOMPETECH}/I%20Got%20a%20Stick%20Arr%20Bryan%20Teoh.mp3"),
    // )
    // .await?;
    // dbg!(&audio1_path);

    // let audio2_path = dl2.run().await?;
    // dbg!(&audio2_path);

    // // extract audio
    // let audio_path = combine.run(audio1_path, audio2_path).await?;
    // dbg!(&audio_path);

    // // copy to example dir so we can test
    // let output_path = PathBuf::from(file!()).with_extension("mp3");
    // tokio::fs::copy(audio_path, output_path).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
}

//     // #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
//     // async fn test_temp_dir_hashing() -> Result<()> {
//     //     use std::hash::Hash;
//     //     let e1 = CombineAudio::new()?;
//     //     let e2 = CombineAudio::new()?;
//     //     assert_eq!((&e1).hash(), (&e2).hash());
//     //     Ok(())
//     // }

//     #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
//     async fn test_concurrent() -> Result<()> {
//         let mut combine = CombineAudio::new()?;
//         let mut dl1 = Download::new(
//             INCOMPETECH.to_owned() + "/I%20Got%20a%20Stick%20Arr%20Bryan%20Teoh.mp3",
//         )?;

//         let mut dl2 = Download::new(INCOMPETECH.to_owned() + "/The%20Ice%20Giants.mp3")?;

//         // download files
//         let audio1_path = dl1.run().await?;
//         dbg!(&audio1_path);

//         let audio2_path = dl2.run().await?;
//         dbg!(&audio2_path);

//         // combine audio files
//         let audio_path = combine.run(audio1_path, audio2_path).await?;
//         dbg!(&audio_path);

//         // copy to example dir so we can listen
//         let output_path = PathBuf::from(file!()).with_extension("mp3");
//         tokio::fs::copy(audio_path, output_path).await?;

//         Ok(())
//     }

//     #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
//     async fn test_serial() -> Result<()> {
//         let mut combine = CombineAudio::new()?;
//         let mut dl1 = Download::new(format!(
//             "{INCOMPETECH}/I%20Got%20a%20Stick%20Arr%20Bryan%20Teoh.mp3"
//         ))?;

//         let mut dl2 = Download::new(format!("{INCOMPETECH}/The%20Ice%20Giants.mp3"))?;

//         // download files
//         let audio1_path = dl1.run().await?;
//         dbg!(&audio1_path);

//         let audio2_path = dl2.run().await?;
//         dbg!(&audio2_path);

//         // combine audio files
//         let audio_path = combine.run(audio1_path, audio2_path).await?;
//         dbg!(&audio_path);

//         // copy to example dir so we can listen
//         let output_path = PathBuf::from(file!()).with_extension("mp3");
//         tokio::fs::copy(audio_path, output_path).await?;

//         Ok(())
//     }
// }

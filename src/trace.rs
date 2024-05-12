use std::collections::HashMap;

use std::time::Instant;

use futures::lock::Mutex;
// use tokio::sync::Mutex;

#[derive(Default, Debug, Clone, Hash, PartialEq, Eq)]
pub struct Task {
    pub label: String,
    pub start: Option<Instant>,
    pub end: Option<Instant>,
}

#[derive(Debug)]
pub struct Trace<T> {
    pub start_time: Instant,
    pub tasks: Mutex<HashMap<T, Task>>,
}

impl<T> Default for Trace<T>
where
    T: std::hash::Hash + std::fmt::Display + std::fmt::Debug + std::cmp::Ord + Eq,
{
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Trace<T>
// where
// T: std::hash::Hash + std::fmt::Display + std::fmt::Debug + std::cmp::Ord + Eq,
{
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            tasks: Mutex::new(HashMap::new()),
        }
    }
}

#[cfg(feature = "render")]
pub mod render {
    use plotters::prelude::*;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    #[inline]
    fn hue_to_rgb(hue: palette::RgbHue) -> RGBColor {
        use palette::IntoColor;
        let hsv = palette::Hsv::new(hue, 1.0, 1.0);
        let rgb: palette::rgb::Rgb = hsv.into_color();
        RGBColor(
            (rgb.red * 255.0) as u8,
            (rgb.green * 255.0) as u8,
            (rgb.blue * 255.0) as u8,
        )
    }

    #[derive(thiserror::Error, Debug)]
    pub enum Error {
        #[error("the trace is too large to be rendered")]
        TooLarge,
        #[error(transparent)]
        Io(#[from] std::io::Error),
    }

    impl<T> super::Trace<T>
    where
        T: std::cmp::Ord + Eq,
    {
        /// Render the execution trace as an SVG image.
        ///
        /// # Errors
        /// - If the trace is too large to be rendered.
        /// - If writing to the specified output path fails.
        pub async fn render_to(&self, path: impl AsRef<std::path::Path>) -> Result<(), Error> {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .open(path.as_ref())?;
            let mut writer = std::io::BufWriter::new(file);
            self.render_to_writer(&mut writer).await
        }

        /// Render the execution trace as an SVG image.
        ///
        /// # Errors
        /// - If the trace is too large to be rendered.
        /// - If writing to the specified output path fails.
        pub async fn render_to_writer(&self, mut writer: impl std::io::Write) -> Result<(), Error> {
            let content = self.render().await?;
            writer.write_all(content.as_bytes())?;
            Ok(())
        }

        /// Render the execution trace as an SVG image.
        ///
        /// # Errors
        /// - If the trace is too large to be rendered.
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_precision_loss)]
        pub async fn render(&self) -> Result<String, Error> {
            #[derive(Default, Debug, Clone)]
            struct Bar<T> {
                begin: u128,
                length: u128,
                label: String,
                id: T,
                color: RGBColor,
            }

            const BAR_HEIGHT: i32 = 40;
            const TARGET_WIDTH: u32 = 2000;

            let tasks = self.tasks.lock().await;

            let mut bars: Vec<_> = tasks
                .iter()
                .filter_map(|(k, t)| match (t.start, t.end) {
                    (Some(s), Some(e)) => {
                        let begin: u128 = s.duration_since(self.start_time).as_millis();
                        let end: u128 = e.duration_since(self.start_time).as_millis();
                        Some(Bar {
                            begin,
                            length: end - begin,
                            label: t.label.clone(),
                            color: RGBColor(0, 0, 0),
                            id: k,
                        })
                    }
                    _ => None,
                })
                .collect();

            // assign colors to the tasks
            let mut rng = ChaCha8Rng::seed_from_u64(0);
            let colors = std::iter::repeat_with(|| {
                let hue = palette::RgbHue::from_degrees(rng.gen_range(0.0..360.0));
                hue_to_rgb(hue)
            });
            bars.sort_by(|a, b| {
                if a.begin == b.begin {
                    a.id.cmp(b.id)
                } else {
                    a.begin.cmp(&b.begin)
                }
            });

            for (bar, color) in bars.iter_mut().zip(colors) {
                bar.color = color;
            }
            // dbg!(&bars);

            // compute the earliest start and latest end time for normalization
            let _earliest = bars.iter().map(|b| b.begin).min();
            let latest = bars.iter().map(|b| b.begin + b.length).max();

            let height = u32::try_from(bars.len()).map_err(|_| Error::TooLarge)?
                * u32::try_from(BAR_HEIGHT).map_err(|_| Error::TooLarge)?
                + 5;
            let bar_width = f64::from(TARGET_WIDTH - 200) / latest.unwrap_or(0) as f64;

            let mut content = String::new();

            {
                let size = (TARGET_WIDTH, height);
                let drawing_area = SVGBackend::with_string(&mut content, size).into_drawing_area();

                let font = ("monospace", BAR_HEIGHT - 10).into_font();
                let text_style = TextStyle::from(font).color(&BLACK);

                for (i, bar) in bars.iter().enumerate() {
                    let i = i32::try_from(i).unwrap();
                    let rect = [
                        ((bar_width * bar.begin as f64) as i32, BAR_HEIGHT * i),
                        (
                            (bar_width * (bar.begin + bar.length) as f64) as i32 + 2,
                            BAR_HEIGHT * (i + 1),
                        ),
                    ];
                    drawing_area
                        .draw(&Rectangle::new(
                            rect,
                            ShapeStyle {
                                color: bar.color.to_rgba(),
                                filled: true,
                                stroke_width: 0,
                            },
                        ))
                        .unwrap();
                    drawing_area
                        .draw_text(
                            &bar.label,
                            &text_style,
                            (
                                (bar_width * bar.begin as f64) as i32 + 1,
                                BAR_HEIGHT * i + 5,
                            ),
                        )
                        .unwrap();
                }
            }
            Ok(content)
        }
    }
}

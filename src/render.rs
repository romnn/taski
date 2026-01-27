#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct Rgba(pub u8, pub u8, pub u8, pub f64);

#[cfg(feature = "render")]
impl Rgba {
    #[must_use]
    pub fn from_hue(hue: palette::RgbHue) -> Self {
        hue_to_rgb(hue).into()
    }
}

#[cfg(feature = "render")]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_truncation)]
impl From<palette::rgb::Rgba> for Rgba {
    fn from(color: palette::rgb::Rgba) -> Self {
        Self(
            (color.red * 255.0) as u8,
            (color.green * 255.0) as u8,
            (color.blue * 255.0) as u8,
            color.alpha.into(),
        )
    }
}

#[cfg(feature = "render")]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_truncation)]
impl From<Rgba> for layout::core::color::Color {
    fn from(color: Rgba) -> Self {
        let Rgba(r, g, b, a) = color;
        Self::new(u32::from_be_bytes([r, g, b, (a * 255.0) as u8]))
    }
}

#[cfg(feature = "render")]
impl From<Rgba> for plotters::prelude::RGBAColor {
    fn from(color: Rgba) -> Self {
        Self(color.0, color.1, color.2, color.3)
    }
}

#[cfg(feature = "render")]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_truncation)]
fn hue_to_rgb(hue: palette::RgbHue) -> palette::rgb::Rgba {
    use palette::IntoColor;
    let hsv = palette::Hsv::new(hue, 1.0, 1.0);
    let rgba: palette::rgb::Rgba = hsv.into_color();
    rgba
}

#[cfg(feature = "render")]
fn calculate_hash<T: std::hash::Hash>(t: T) -> u64 {
    use std::hash::Hasher;
    let mut s = std::hash::DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

/// Compute a color based on the hash of the given ID.
#[cfg(feature = "render")]
pub fn color_from_id<I: std::hash::Hash>(id: I) -> Rgba {
    use rand::{Rng, SeedableRng};
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(calculate_hash(id));
    let hue = palette::RgbHue::from_degrees(rng.random_range(0.0..360.0));
    hue_to_rgb(hue).into()
}

#[cfg(feature = "render")]
pub mod trace {
    use plotters::prelude::*;
    use std::time::Duration;

    #[derive(thiserror::Error, Debug)]
    pub enum Error {
        #[error("the trace is too large to be rendered")]
        TooLarge,
        #[error(transparent)]
        Io(#[from] std::io::Error),
    }

    impl<T> crate::trace::Trace<T>
    where
        T: std::hash::Hash + std::cmp::Ord + Eq,
    {
        /// Render the execution trace as an SVG image.
        ///
        /// # Errors
        /// - If the trace is too large to be rendered.
        /// - If writing to the specified output path fails.
        pub fn render_to(&self, path: impl AsRef<std::path::Path>) -> Result<(), Error> {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .open(path.as_ref())?;
            let mut writer = std::io::BufWriter::new(file);
            self.render_to_writer(&mut writer)
        }

        /// Render the execution trace as an SVG image.
        ///
        /// # Errors
        /// - If the trace is too large to be rendered.
        /// - If writing to the specified output path fails.
        pub fn render_to_writer(&self, mut writer: impl std::io::Write) -> Result<(), Error> {
            let content = self.render()?;
            writer.write_all(content.as_bytes())?;
            Ok(())
        }

        /// Render the execution trace as an SVG image.
        ///
        /// # Errors
        /// - If the trace is too large to be rendered.
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_precision_loss)]
        pub fn render(&self) -> Result<String, Error> {
            #[derive(Default, Debug, Clone)]
            struct Bar<T> {
                begin: u128,
                length: u128,
                label: String,
                id: Option<T>,
                color: RGBAColor,
            }

            const BAR_HEIGHT: i32 = 40;
            const TARGET_WIDTH: u32 = 2000;

            let mut bars: Vec<_> = self
                .tasks
                .iter()
                .map(|(k, t)| {
                    let begin: u128 = t.start.duration_since(self.start_time).as_nanos();
                    let end: u128 = t.end.duration_since(self.start_time).as_nanos();
                    let color = t.color.map_or(RGBAColor(0, 200, 255, 1.0), Into::into);
                    Bar {
                        begin,
                        length: end.saturating_sub(begin),
                        label: t.label.clone(),
                        color,
                        id: Some(k),
                    }
                })
                .collect();

            // compute the latest end time for normalization
            let latest = bars
                .iter()
                .map(|b| b.begin + b.length)
                .max()
                .unwrap_or_default();

            // compute the total duration of the trace.
            let total_duration = Duration::from_nanos(latest.try_into().unwrap());

            bars.push(Bar {
                begin: 0,
                length: latest,
                label: format!("Total ({total_duration:.2?})"),
                color: RGBAColor(200, 200, 200, 1.0),
                id: None,
            });

            // sort bars based based on their time
            bars.sort_by(|a, b| {
                if a.begin == b.begin {
                    a.id.cmp(&b.id)
                } else {
                    a.begin.cmp(&b.begin)
                }
            });

            let height = u32::try_from(bars.len()).map_err(|_| Error::TooLarge)?
                * u32::try_from(BAR_HEIGHT).map_err(|_| Error::TooLarge)?
                + 5;
            let bar_width = f64::from(TARGET_WIDTH - 200) / latest as f64;

            let mut content = String::new();

            {
                let size = (TARGET_WIDTH, height);
                let drawing_area = SVGBackend::with_string(&mut content, size).into_drawing_area();

                let font = ("monospace", BAR_HEIGHT - 10).into_font();
                let text_style = TextStyle::from(font).color(&BLACK);

                for (i, bar) in bars.iter().enumerate() {
                    let i = i32::try_from(i).unwrap();

                    let top_left = ((bar_width * bar.begin as f64) as i32, BAR_HEIGHT * i);
                    let bottom_right = (
                        (bar_width * (bar.begin + bar.length) as f64) as i32 + 2,
                        BAR_HEIGHT * (i + 1),
                    );

                    // draw bar
                    drawing_area
                        .draw(&Rectangle::new(
                            [top_left, bottom_right],
                            ShapeStyle {
                                color: bar.color.to_rgba(),
                                filled: true,
                                stroke_width: 0,
                            },
                        ))
                        .unwrap();

                    // draw label
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

#[cfg(feature = "render")]
pub mod schedule {
    use crate::{
        schedule::{Schedulable, Schedule},
        task,
    };

    use layout::{
        backends::svg::SVGWriter,
        core::{self, base::Orientation, color::Color, style},
        std_shapes::shapes,
        topo::layout::VisualGraph,
    };
    use petgraph as pg;
    use std::collections::HashMap;
    use std::sync::Arc;

    impl<L> Schedule<L> {
        /// Render the task graph as an svg image.
        ///
        /// # Errors
        /// - If writing to the specified output path fails.
        pub fn render_to(&self, path: impl AsRef<std::path::Path>) -> Result<(), std::io::Error> {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .open(path.as_ref())?;
            let mut writer = std::io::BufWriter::new(file);
            self.render_to_writer(&mut writer)
        }

        /// Render the task graph as an svg image.
        ///
        /// # Errors
        /// - If writing to the specified output path fails.
        pub fn render_to_writer(
            &self,
            mut writer: impl std::io::Write,
        ) -> Result<(), std::io::Error> {
            let content = self.render();
            writer.write_all(content.as_bytes())?;
            Ok(())
        }

        /// Render the task graph as an svg image.
        #[must_use]
        pub fn render(&self) -> String {
            fn node<LL>(node: &task::Ref<LL>) -> shapes::Element {
                let node_style = style::StyleAttr {
                    line_color: Color::new(0x0000_00FF),
                    line_width: 2,
                    fill_color: node.color().map(Into::into),
                    rounded: 0,
                    font_size: 15,
                };
                let size = core::geometry::Point { x: 100.0, y: 100.0 };
                shapes::Element::create(
                    shapes::ShapeKind::Circle(format!("{node}")),
                    node_style,
                    Orientation::TopToBottom,
                    size,
                )
            }

            let mut graph = VisualGraph::new(Orientation::TopToBottom);

            let mut handles: HashMap<Arc<dyn Schedulable<L>>, layout::adt::dag::NodeHandle> =
                HashMap::new();

            for idx in self.dag.node_indices() {
                let task = &self.dag[idx];
                let deps = self
                    .dag
                    .neighbors_directed(idx, pg::Direction::Incoming)
                    .map(|idx| &self.dag[idx]);

                let dest_handle = *handles
                    .entry(task.clone())
                    .or_insert_with(|| graph.add_node(node(task)));
                for dep in deps {
                    let src_handle = *handles
                        .entry(dep.clone())
                        .or_insert_with(|| graph.add_node(node(dep)));
                    let arrow = shapes::Arrow {
                        start: shapes::LineEndKind::None,
                        end: shapes::LineEndKind::Arrow,
                        line_style: style::LineStyleKind::Normal,
                        text: String::new(),
                        look: style::StyleAttr {
                            line_color: Color::new(0x0000_00FF),
                            line_width: 2,
                            fill_color: Some(Color::new(0xB4B3_B2FF)),
                            rounded: 0,
                            font_size: 15,
                        },
                        properties: None,
                        src_port: None,
                        dst_port: None,
                    };
                    graph.add_edge(arrow, src_handle, dest_handle);
                }
            }

            // https://docs.rs/layout-rs/latest/src/layout/backends/svg.rs.html#200
            let mut backend = SVGWriter::new();
            let debug_mode = false;
            let disable_opt = false;
            let disable_layout = false;
            graph.do_it(debug_mode, disable_opt, disable_layout, &mut backend);
            backend.finalize()
        }
    }
}

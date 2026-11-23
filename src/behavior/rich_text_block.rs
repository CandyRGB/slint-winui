use std::ops::Range;

use slint::{Color, Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};
use slint_winui::platform::rich_text as native;

use crate::{
    RichTextAlignment, RichTextBehavior, RichTextFill, RichTextFormat, RichTextFrame,
    RichTextHighlight, RichTextHit, RichTextImage, RichTextLayout, RichTextLink, RichTextLocation,
    RichTextNavigation, RichTextParagraph, RichTextRect, RichTextRun,
};

pub fn install(behavior: RichTextBehavior<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let renderer = native::Renderer::new()?;
    behavior.on_layout(move |paragraphs, highlights, regions, format| {
        layout(&renderer, paragraphs, highlights, regions, format)
    });
    behavior.on_slice(|text, start, end| {
        let wide: Vec<_> = text.encode_utf16().collect();
        slice(&wide, normalized_range(start, end)).into()
    });
    behavior.on_select_rectangles(|hits, start, end| {
        model(merged_rectangles(hits.iter(), normalized_range(start, end)))
    });
    behavior.on_hit_test(|frames, point| hit_test(frames, point.x, point.y));
    behavior.on_step(step);
    Ok(())
}

struct Link {
    range: Range<u32>,
    label: SharedString,
    target: SharedString,
}

struct Attachment {
    position: u32,
    width: f32,
    height: f32,
    image: Image,
    label: SharedString,
}

#[derive(Default)]
struct Document {
    paragraphs: Vec<native::Paragraph>,
    wide: Vec<u16>,
    links: Vec<Link>,
    attachments: Vec<Attachment>,
}

impl Document {
    fn from_paragraphs(paragraphs: ModelRc<RichTextParagraph>, link_color: Color) -> Self {
        let mut document = Self::default();
        for paragraph in paragraphs.iter() {
            if !document.paragraphs.is_empty() {
                document.wide.push(b'\n' as u16);
            }
            let start = document.wide.len() as u32;
            let mut native_paragraph = native::Paragraph::default();
            for run in paragraph.runs.iter() {
                document.push_run(&mut native_paragraph, run, start, link_color);
            }
            document.wide.extend_from_slice(&native_paragraph.text);
            document.paragraphs.push(native_paragraph);
        }
        document
    }

    fn push_run(
        &mut self,
        paragraph: &mut native::Paragraph,
        run: RichTextRun,
        base: u32,
        link_color: Color,
    ) {
        let start = paragraph.text.len() as u32;
        let mut inline_size = None;
        let image_size = run.image.size();
        if image_size.width > 0 && image_size.height > 0 {
            let width = if run.image_width > 0.0 {
                run.image_width
            } else {
                image_size.width as f32
            };
            let height = if run.image_height > 0.0 {
                run.image_height
            } else {
                image_size.height as f32
            };
            inline_size = Some((width, height));
            self.attachments.push(Attachment {
                position: base + start,
                width,
                height,
                image: run.image,
                label: run.image_label.clone(),
            });
            paragraph.text.push(0xfffc);
        } else {
            paragraph.text.extend(run.text.encode_utf16());
        }
        let end = paragraph.text.len() as u32;
        if start == end {
            return;
        }
        if !run.link.is_empty() {
            self.links.push(Link {
                range: (base + start)..(base + end),
                label: if inline_size.is_some() {
                    run.image_label
                } else {
                    run.text
                },
                target: run.link.clone(),
            });
        }
        paragraph.spans.push(native::Span {
            range: start..end,
            font_family: run.font_family.to_string(),
            font_size: run.font_size,
            font_weight: run.font_weight.clamp(0, 999) as u16,
            italic: run.italic,
            underline: run.underline || !run.link.is_empty(),
            strikethrough: run.strikethrough,
            foreground: if run.custom_foreground {
                Some(rgba(run.foreground))
            } else if run.link.is_empty() {
                None
            } else {
                Some(rgba(link_color))
            },
            inline_size,
        });
    }

    fn apply_highlight_colors(&mut self, highlights: &ModelRc<RichTextHighlight>) {
        let mut base = 0;
        for paragraph in &mut self.paragraphs {
            let end = base + paragraph.text.len() as u32;
            for entry in highlights.iter().filter(|entry| entry.custom_foreground) {
                let range = highlight_range(&entry);
                let start = range.start.max(base);
                let stop = range.end.min(end);
                if start < stop {
                    paragraph.spans.push(native::Span {
                        range: (start - base)..(stop - base),
                        foreground: Some(rgba(entry.foreground)),
                        ..Default::default()
                    });
                }
            }
            base = end + 1;
        }
    }
}

fn layout(
    renderer: &native::Renderer,
    paragraphs: ModelRc<RichTextParagraph>,
    highlights: ModelRc<RichTextHighlight>,
    regions: ModelRc<RichTextRect>,
    format: RichTextFormat,
) -> RichTextLayout {
    let mut document = Document::from_paragraphs(paragraphs, format.link_color);
    document.apply_highlight_colors(&highlights);
    let requested: Vec<_> = regions
        .iter()
        .map(|r| native::Region {
            width: r.width,
            height: r.height,
        })
        .collect();
    let base = RichTextLayout {
        text: String::from_utf16_lossy(&document.wide).into(),
        length: document.wide.len() as i32,
        ..Default::default()
    };
    let native_format = native::Format {
        family: &format.font_family,
        size: format.font_size,
        weight: format.font_weight.clamp(1, 999) as u16,
        foreground: rgba(format.foreground),
        alignment: match format.alignment {
            RichTextAlignment::Leading => native::Alignment::Leading,
            RichTextAlignment::Center => native::Alignment::Center,
            RichTextAlignment::Trailing => native::Alignment::Trailing,
            RichTextAlignment::Justified => native::Alignment::Justified,
        },
        wrap: format.wraps,
        rtl: format.rtl,
        paragraph_spacing: format.paragraph_spacing,
    };
    let scale = if format.scale_factor > 0.0 {
        format.scale_factor
    } else {
        1.0
    };
    let Ok(flow) = renderer.render(&document.paragraphs, &requested, &native_format, scale) else {
        return RichTextLayout {
            has_overflow: base.length > 0,
            ..base
        };
    };
    let frames: Vec<_> = regions
        .iter()
        .zip(flow.frames)
        .map(|(region, frame)| present(frame, region, &document, &highlights))
        .collect();
    let height = frames
        .iter()
        .map(|frame| frame.region.y + frame.region.height)
        .fold(0.0, f32::max);
    RichTextLayout {
        frames: model(frames),
        height,
        has_overflow: flow.has_overflow,
        consumed: flow.consumed as i32,
        ..base
    }
}

fn present(
    frame: native::Frame,
    region: RichTextRect,
    document: &Document,
    highlights: &ModelRc<RichTextHighlight>,
) -> RichTextFrame {
    let ink = if frame.pixels.is_empty() {
        Image::default()
    } else {
        let image = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
            &frame.pixels,
            frame.pixel_width,
            frame.pixel_height,
        );
        Image::from_rgba8_premultiplied(image)
    };
    let hits: Vec<_> = frame
        .hits
        .iter()
        .map(|hit| RichTextHit {
            start: hit.range.start as i32,
            end: hit.range.end as i32,
            bounds: rect(hit.rect),
            rtl: hit.rtl,
        })
        .collect();
    let fills = highlights
        .iter()
        .flat_map(|highlight| {
            merged_rectangles(hits.iter().cloned(), highlight_range(&highlight))
                .into_iter()
                .map(move |bounds| RichTextFill {
                    bounds,
                    color: highlight.background,
                })
        })
        .collect();
    let links = document
        .links
        .iter()
        .filter_map(|link| {
            let bounds = merged_rectangles(hits.iter().cloned(), link.range.clone())
                .into_iter()
                .next()?;
            Some(RichTextLink {
                bounds,
                start: link.range.start as i32,
                end: link.range.end as i32,
                label: link.label.clone(),
                target: link.target.clone(),
            })
        })
        .collect();
    let images = document
        .attachments
        .iter()
        .filter_map(|attachment| {
            let hit = frame
                .hits
                .iter()
                .find(|hit| hit.range.contains(&attachment.position))?;
            Some(RichTextImage {
                bounds: RichTextRect {
                    x: hit.rect.x,
                    y: hit.baseline - attachment.height,
                    width: attachment.width,
                    height: attachment.height,
                },
                source: attachment.image.clone(),
                label: attachment.label.clone(),
            })
        })
        .collect();
    let text = match (hits.first(), hits.last()) {
        (Some(first), Some(last)) => {
            slice(&document.wide, first.start as u32..last.end as u32).into()
        }
        _ => SharedString::default(),
    };
    RichTextFrame {
        region: RichTextRect {
            height: if region.height > 0.0 {
                region.height
            } else {
                frame.height
            },
            ..region
        },
        ink,
        ink_height: frame.height,
        hits: model(hits),
        fills: model(fills),
        images: model(images),
        links: model(links),
        text,
    }
}

fn rect(rect: native::Rect) -> RichTextRect {
    RichTextRect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

fn rgba(color: Color) -> native::Rgba {
    [color.red(), color.green(), color.blue(), color.alpha()]
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

fn normalized_range(a: i32, b: i32) -> Range<u32> {
    a.min(b).max(0) as u32..a.max(b).max(0) as u32
}

fn highlight_range(highlight: &RichTextHighlight) -> Range<u32> {
    let start = highlight.start.max(0) as u32;
    start..start.saturating_add(highlight.length.max(0) as u32)
}

fn slice(text: &[u16], range: Range<u32>) -> String {
    let mut start = (range.start as usize).min(text.len());
    let mut end = (range.end as usize).min(text.len());
    if start == end {
        return String::new();
    }
    if (0xdc00..=0xdfff).contains(&text[start]) {
        start = start.saturating_sub(1);
    }
    if text
        .get(end)
        .is_some_and(|unit| (0xdc00..=0xdfff).contains(unit))
    {
        end += 1;
    }
    String::from_utf16_lossy(&text[start..end])
}

fn included(hit: &RichTextHit, range: &Range<u32>) -> bool {
    (hit.start as u32) < range.end && (hit.end as u32) > range.start
}

fn merged_rectangles(
    hits: impl Iterator<Item = RichTextHit>,
    range: Range<u32>,
) -> Vec<RichTextRect> {
    if range.is_empty() {
        return Vec::new();
    }
    let mut rectangles: Vec<_> = hits
        .filter(|hit| included(hit, &range) && hit.bounds.width > 0.0)
        .map(|hit| hit.bounds)
        .collect();
    rectangles.sort_by(|a, b| a.y.total_cmp(&b.y).then_with(|| a.x.total_cmp(&b.x)));
    let mut merged: Vec<RichTextRect> = Vec::new();
    for next in rectangles {
        if let Some(last) = merged.last_mut() {
            if (last.y - next.y).abs() < 0.1
                && (last.height - next.height).abs() < 0.1
                && next.x <= last.x + last.width + 0.1
            {
                last.width = (next.x + next.width - last.x).max(last.width);
                continue;
            }
        }
        merged.push(next);
    }
    merged
}

fn distance(bounds: &RichTextRect, x: f32, y: f32) -> f32 {
    let dx = (bounds.x - x).max(x - bounds.x - bounds.width).max(0.0);
    let dy = (bounds.y - y).max(y - bounds.y - bounds.height).max(0.0);
    dx * dx + 4.0 * dy * dy
}

fn hit_test(frames: ModelRc<RichTextFrame>, x: f32, y: f32) -> RichTextLocation {
    let frame = frames
        .iter()
        .filter(|frame| frame.hits.row_count() != 0)
        .min_by(|a, b| distance(&a.region, x, y).total_cmp(&distance(&b.region, x, y)));
    let Some(frame) = frame else {
        return RichTextLocation::default();
    };
    let x = x - frame.region.x;
    let y = y - frame.region.y;
    let best = frame
        .hits
        .iter()
        .min_by(|a, b| distance(&a.bounds, x, y).total_cmp(&distance(&b.bounds, x, y)));
    let Some(hit) = best else {
        return RichTextLocation::default();
    };
    let trailing = (x >= hit.bounds.x + hit.bounds.width / 2.0) != hit.rtl;
    let link = (distance(&hit.bounds, x, y) == 0.0)
        .then(|| {
            frame
                .links
                .iter()
                .find(|link| link.start < hit.end && link.end > hit.start)
                .map(|link| link.target)
                .unwrap_or_default()
        })
        .unwrap_or_default();
    RichTextLocation {
        offset: if trailing { hit.end } else { hit.start },
        link,
    }
}

fn step(frames: ModelRc<RichTextFrame>, offset: i32, direction: RichTextNavigation) -> i32 {
    use RichTextNavigation::*;

    if matches!(direction, Previous | Next) {
        let boundaries = frames.iter().flat_map(|frame| {
            frame
                .hits
                .iter()
                .flat_map(|hit| [hit.start, hit.end])
                .collect::<Vec<_>>()
        });
        return match direction {
            Next => boundaries.filter(|boundary| *boundary > offset).min(),
            _ => boundaries.filter(|boundary| *boundary < offset).max(),
        }
        .unwrap_or(offset);
    }

    let mut lines: Vec<Vec<RichTextHit>> = Vec::new();
    for frame in frames.iter() {
        let mut previous_y: Option<f32> = None;
        for hit in frame.hits.iter() {
            if previous_y.is_none_or(|y| (y - hit.bounds.y).abs() > 0.1) {
                lines.push(Vec::new());
            }
            previous_y = Some(hit.bounds.y);
            lines.last_mut().unwrap().push(hit);
        }
    }
    if lines.is_empty() {
        return offset;
    }
    // A paragraph break belongs to the preceding line. A wrapped boundary
    // shared by two lines belongs to the start of the next line.
    let line_index = lines
        .partition_point(|line| line.first().unwrap().start <= offset)
        .saturating_sub(1);
    let line = &lines[line_index];
    match direction {
        LineStart => line.first().unwrap().start,
        LineEnd => line.last().unwrap().end,
        Up | Down => {
            let target = if direction == Up {
                line_index.checked_sub(1)
            } else {
                Some(line_index + 1)
            };
            let Some(target) = target.and_then(|index| lines.get(index)) else {
                return offset;
            };
            let active = line
                .iter()
                .find(|hit| offset < hit.end)
                .unwrap_or_else(|| line.last().unwrap());
            let x = caret_x(active, offset >= active.end);
            target
                .iter()
                .flat_map(|hit| {
                    [
                        (hit.start, caret_x(hit, false)),
                        (hit.end, caret_x(hit, true)),
                    ]
                })
                .min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs()))
                .map_or(offset, |(boundary, _)| boundary)
        }
        _ => offset,
    }
}

fn caret_x(hit: &RichTextHit, trailing: bool) -> f32 {
    if trailing != hit.rtl {
        hit.bounds.x + hit.bounds.width
    } else {
        hit.bounds.x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_keeps_cluster_boundaries_across_regions() {
        let hit = |start, end, x, y| RichTextHit {
            start,
            end,
            bounds: RichTextRect {
                x,
                y,
                width: 10.0,
                height: 16.0,
            },
            rtl: false,
        };
        let frames = model(vec![
            RichTextFrame {
                hits: model(vec![
                    hit(0, 1, 0.0, 0.0),
                    hit(1, 3, 10.0, 0.0),
                    hit(3, 4, 0.0, 16.0),
                ]),
                ..Default::default()
            },
            RichTextFrame {
                hits: model(vec![hit(5, 6, 0.0, 0.0), hit(6, 7, 10.0, 0.0)]),
                ..Default::default()
            },
        ]);
        use RichTextNavigation::*;
        assert_eq!(step(frames.clone(), 1, Next), 3);
        assert_eq!(step(frames.clone(), 3, Previous), 1);
        assert_eq!(step(frames.clone(), 1, LineEnd), 3);
        assert_eq!(step(frames.clone(), 6, LineStart), 5);
        assert_eq!(step(frames.clone(), 1, Down), 4);
        assert_eq!(step(frames.clone(), 4, Down), 6);
        assert_eq!(step(frames, 5, Up), 3);
    }
}

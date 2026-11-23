use std::ffi::c_void;
use std::ops::Range;
use std::ptr::null;

use windows::Win32::Foundation::{E_INVALIDARG, RPC_E_CHANGED_MODE};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_SOFTWARE, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, D2D1CreateFactory,
    ID2D1Factory, ID2D1RenderTarget,
};
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
    WICBitmapCacheOnLoad,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::core::{BOOL, Error, HSTRING, IUnknown, Interface, Ref, Result, implement, w};
use windows_numerics::Vector2;

pub type Rgba = [u8; 4];

#[derive(Clone, Copy, Default)]
pub enum Alignment {
    #[default]
    Leading,
    Center,
    Trailing,
    Justified,
}

#[derive(Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Default)]
pub struct Span {
    pub range: Range<u32>,
    pub font_family: String,
    pub font_size: f32,
    pub font_weight: u16,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub foreground: Option<Rgba>,
    pub inline_size: Option<(f32, f32)>,
}

#[derive(Default)]
pub struct Paragraph {
    pub text: Vec<u16>,
    pub spans: Vec<Span>,
}

pub struct Format<'a> {
    pub family: &'a str,
    pub size: f32,
    pub weight: u16,
    pub foreground: Rgba,
    pub alignment: Alignment,
    pub wrap: bool,
    pub rtl: bool,
    pub paragraph_spacing: f32,
}

#[derive(Clone)]
pub struct Hit {
    pub range: Range<u32>,
    pub rect: Rect,
    pub baseline: f32,
    pub rtl: bool,
}

#[derive(Clone, Copy)]
pub struct Region {
    pub width: f32,
    /// Zero allows the region to grow to the height of its content.
    pub height: f32,
}

#[derive(Default)]
pub struct Frame {
    pub pixels: Vec<u8>,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub height: f32,
    pub hits: Vec<Hit>,
}

#[derive(Default)]
pub struct Flow {
    pub frames: Vec<Frame>,
    pub consumed: u32,
    pub has_overflow: bool,
}

#[derive(Default)]
struct Position {
    paragraph: usize,
    offset: u32,
    paragraph_start: u32,
}

struct Fragment<'a> {
    layout: IDWriteTextLayout,
    paragraph: &'a Paragraph,
    offset: u32,
    absolute_start: u32,
    consumed: u32,
    y: f32,
    height: f32,
    lines: Vec<DWRITE_LINE_METRICS>,
}

pub struct Renderer {
    direct_write: IDWriteFactory,
    direct_2d: ID2D1Factory,
    imaging: IWICImagingFactory,
    // Keep this last so the COM objects are released before balancing CoInitializeEx.
    _apartment: Apartment,
}

struct Apartment(bool);

impl Apartment {
    fn enter() -> Result<Self> {
        // Only this thread's COM initialization count is changed.
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if result == RPC_E_CHANGED_MODE {
            return Ok(Self(false));
        }
        result.ok()?;
        Ok(Self(true))
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            // This guard is held by the UI-thread renderer and balances the successful call above.
            unsafe { CoUninitialize() };
        }
    }
}

impl Renderer {
    pub fn new() -> Result<Self> {
        let apartment = Apartment::enter()?;
        // Factories are created and used on the same thread.
        unsafe {
            Ok(Self {
                direct_write: DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?,
                direct_2d: D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?,
                imaging: CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?,
                _apartment: apartment,
            })
        }
    }

    pub fn render(
        &self,
        paragraphs: &[Paragraph],
        regions: &[Region],
        format: &Format<'_>,
        scale: f32,
    ) -> Result<Flow> {
        let mut position = Position::default();
        let mut frames = Vec::with_capacity(regions.len());
        for region in regions {
            if !region.width.is_finite()
                || !region.height.is_finite()
                || region.width < 0.0
                || region.height < 0.0
            {
                return Err(Error::from_hresult(E_INVALIDARG));
            }
            let fragments = self.flow_region(paragraphs, &mut position, *region, format)?;
            frames.push(self.paint_region(*region, format, &fragments, scale)?);
        }
        Ok(Flow {
            frames,
            consumed: position.paragraph_start + position.offset,
            has_overflow: position.paragraph < paragraphs.len(),
        })
    }

    fn flow_region<'a>(
        &self,
        paragraphs: &'a [Paragraph],
        position: &mut Position,
        region: Region,
        format: &Format<'_>,
    ) -> Result<Vec<Fragment<'a>>> {
        let mut fragments = Vec::new();
        if region.width <= 0.0 {
            return Ok(fragments);
        }
        let limit = if region.height == 0.0 {
            f32::INFINITY
        } else {
            region.height
        };
        let mut y = 0.0;
        while let Some(paragraph) = paragraphs.get(position.paragraph) {
            let layout = self.shape(paragraph, position.offset, region.width, format)?;
            let lines = line_metrics(&layout)?;
            let mut height = 0.0;
            let mut consumed = 0;
            let mut visible = 0;
            for line in &lines {
                if y + height + line.height > limit + 0.01 {
                    break;
                }
                height += line.height;
                consumed += line.length;
                visible += 1;
            }
            if visible == 0 {
                break;
            }
            fragments.push(Fragment {
                layout,
                paragraph,
                offset: position.offset,
                absolute_start: position.paragraph_start + position.offset,
                consumed,
                y,
                height,
                lines: lines[..visible].to_vec(),
            });
            position.offset += consumed;
            if position.offset < paragraph.text.len() as u32 {
                break;
            }
            position.paragraph += 1;
            position.paragraph_start += paragraph.text.len() as u32;
            if position.paragraph < paragraphs.len() {
                position.paragraph_start += 1;
            }
            position.offset = 0;
            y += height + format.paragraph_spacing.max(0.0);
        }
        Ok(fragments)
    }

    fn shape(
        &self,
        paragraph: &Paragraph,
        offset: u32,
        width: f32,
        format: &Format<'_>,
    ) -> Result<IDWriteTextLayout> {
        let family = HSTRING::from(if format.family.is_empty() {
            "Segoe UI Variable Text"
        } else {
            format.family
        });
        let remaining = &paragraph.text[(offset as usize)..];
        // The layout holds any native style and inline-object references assigned to its ranges.
        unsafe {
            let base = self.direct_write.CreateTextFormat(
                &family,
                None,
                DWRITE_FONT_WEIGHT(i32::from(format.weight).clamp(1, 999)),
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                format.size.max(1.0),
                w!("en-US"),
            )?;
            base.SetReadingDirection(if format.rtl {
                DWRITE_READING_DIRECTION_RIGHT_TO_LEFT
            } else {
                DWRITE_READING_DIRECTION_LEFT_TO_RIGHT
            })?;
            base.SetTextAlignment(match format.alignment {
                Alignment::Leading => DWRITE_TEXT_ALIGNMENT_LEADING,
                Alignment::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
                Alignment::Trailing => DWRITE_TEXT_ALIGNMENT_TRAILING,
                Alignment::Justified => DWRITE_TEXT_ALIGNMENT_JUSTIFIED,
            })?;
            base.SetWordWrapping(if format.wrap {
                DWRITE_WORD_WRAPPING_WRAP
            } else {
                DWRITE_WORD_WRAPPING_NO_WRAP
            })?;
            let layout = self
                .direct_write
                .CreateTextLayout(remaining, &base, width, f32::MAX)?;
            for span in &paragraph.spans {
                let Some(range) =
                    intersect(&span.range, offset..paragraph.text.len() as u32, offset)
                else {
                    continue;
                };
                if !span.font_family.is_empty() {
                    layout.SetFontFamilyName(&HSTRING::from(&span.font_family), range)?;
                }
                if span.font_size > 0.0 {
                    layout.SetFontSize(span.font_size, range)?;
                }
                if span.font_weight != 0 {
                    layout.SetFontWeight(
                        DWRITE_FONT_WEIGHT(i32::from(span.font_weight).min(999)),
                        range,
                    )?;
                }
                if span.italic {
                    layout.SetFontStyle(DWRITE_FONT_STYLE_ITALIC, range)?;
                }
                if span.underline {
                    layout.SetUnderline(true, range)?;
                }
                if span.strikethrough {
                    layout.SetStrikethrough(true, range)?;
                }
                if let Some((width, height)) = span.inline_size {
                    let object: IDWriteInlineObject = InlineSpace { width, height }.into();
                    layout.SetInlineObject(&object, range)?;
                }
            }
            Ok(layout)
        }
    }

    fn paint_region(
        &self,
        region: Region,
        format: &Format<'_>,
        fragments: &[Fragment<'_>],
        scale: f32,
    ) -> Result<Frame> {
        let Some(last) = fragments.last() else {
            return Ok(Frame::default());
        };
        let height = last.y + last.height;
        let pixel_width = (region.width * scale).ceil() as u32;
        let pixel_height = (height * scale).ceil() as u32;
        // A WIC render surface must be nonempty and fit within the render-target limit.
        if !(1..=16_384).contains(&pixel_width) || !(1..=16_384).contains(&pixel_height) {
            return Err(Error::from_hresult(E_INVALIDARG));
        }
        let mut hits = Vec::new();
        for fragment in fragments {
            collect_hits(fragment, &mut hits)?;
        }
        // WIC owns the isolated BGRA surface. It is transparent wherever no glyph was painted.
        unsafe {
            let bitmap = self.imaging.CreateBitmap(
                pixel_width,
                pixel_height,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapCacheOnLoad,
            )?;
            let target = self.direct_2d.CreateWicBitmapRenderTarget(
                &bitmap,
                &D2D1_RENDER_TARGET_PROPERTIES {
                    r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    dpiX: 96.0 * scale,
                    dpiY: 96.0 * scale,
                    ..Default::default()
                },
            )?;
            let ink = target.CreateSolidColorBrush(&color(format.foreground), None)?;
            for fragment in fragments {
                span_colors(&target, fragment)?;
            }
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            target.BeginDraw();
            target.Clear(Some(&D2D1_COLOR_F::default()));
            for fragment in fragments {
                let clip = D2D_RECT_F {
                    left: 0.0,
                    top: fragment.y,
                    right: region.width,
                    bottom: fragment.y + fragment.height,
                };
                target.PushAxisAlignedClip(&clip, D2D1_ANTIALIAS_MODE_ALIASED);
                target.DrawTextLayout(
                    Vector2 {
                        X: 0.0,
                        Y: fragment.y,
                    },
                    &fragment.layout,
                    &ink,
                    D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                );
                target.PopAxisAlignedClip();
            }
            target.EndDraw(None, None)?;
            let stride = pixel_width * 4;
            let mut pixels = vec![0; stride as usize * pixel_height as usize];
            bitmap.CopyPixels(null(), stride, &mut pixels)?;
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            Ok(Frame {
                pixels,
                pixel_width,
                pixel_height,
                height,
                hits,
            })
        }
    }
}

fn intersect(range: &Range<u32>, region: Range<u32>, origin: u32) -> Option<DWRITE_TEXT_RANGE> {
    let start = range.start.max(region.start);
    let end = range.end.min(region.end);
    (start < end).then(|| DWRITE_TEXT_RANGE {
        startPosition: start - origin,
        length: end - start,
    })
}

fn color([r, g, b, a]: Rgba) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: f32::from(r) / 255.0,
        g: f32::from(g) / 255.0,
        b: f32::from(b) / 255.0,
        a: f32::from(a) / 255.0,
    }
}

fn line_metrics(layout: &IDWriteTextLayout) -> Result<Vec<DWRITE_LINE_METRICS>> {
    let mut count = 0;
    // The first call reports the required buffer length even when no storage was supplied.
    unsafe {
        let result = layout.GetLineMetrics(None, &mut count);
        if count == 0 {
            result?;
            return Ok(Vec::new());
        }
        let mut lines = vec![DWRITE_LINE_METRICS::default(); count as usize];
        layout.GetLineMetrics(Some(&mut lines), &mut count)?;
        lines.truncate(count as usize);
        Ok(lines)
    }
}

fn collect_hits(fragment: &Fragment<'_>, hits: &mut Vec<Hit>) -> Result<()> {
    let mut count = 0;
    // DWrite's cluster lengths and hit-test positions both use UTF-16 code units.
    unsafe {
        let result = fragment.layout.GetClusterMetrics(None, &mut count);
        if count == 0 {
            return result;
        }
        let mut clusters = vec![DWRITE_CLUSTER_METRICS::default(); count as usize];
        fragment
            .layout
            .GetClusterMetrics(Some(&mut clusters), &mut count)?;
        let mut position = 0;
        let mut line = 0;
        let mut line_end = fragment.lines.first().map_or(0, |metrics| metrics.length);
        for cluster in clusters.into_iter().take(count as usize) {
            if position >= fragment.consumed {
                break;
            }
            while position >= line_end && line + 1 < fragment.lines.len() {
                line += 1;
                line_end += fragment.lines[line].length;
            }
            let mut x = 0.0;
            let mut y = 0.0;
            let mut hit = DWRITE_HIT_TEST_METRICS::default();
            fragment
                .layout
                .HitTestTextPosition(position, false, &mut x, &mut y, &mut hit)?;
            let length = u32::from(cluster.length);
            let start = fragment.absolute_start + position;
            hits.push(Hit {
                range: start..start + length,
                rect: Rect {
                    x: hit.left,
                    y: hit.top + fragment.y,
                    width: hit.width,
                    height: hit.height,
                },
                baseline: hit.top + fragment.y + fragment.lines[line].baseline,
                rtl: hit.bidiLevel % 2 == 1,
            });
            position += length;
        }
        Ok(())
    }
}

fn span_colors(target: &ID2D1RenderTarget, fragment: &Fragment<'_>) -> Result<()> {
    for span in &fragment.paragraph.spans {
        let Some(foreground) = span.foreground else {
            continue;
        };
        let Some(range) = intersect(
            &span.range,
            fragment.offset..fragment.offset + fragment.consumed,
            fragment.offset,
        ) else {
            continue;
        };
        // The drawing effect is a brush from this render target and stays alive through the layout.
        unsafe {
            let brush = target.CreateSolidColorBrush(&color(foreground), None)?;
            fragment
                .layout
                .SetDrawingEffect(&brush.cast::<IUnknown>()?, range)?;
        }
    }
    Ok(())
}

#[implement(IDWriteInlineObject)]
struct InlineSpace {
    width: f32,
    height: f32,
}

impl IDWriteInlineObject_Impl for InlineSpace_Impl {
    fn Draw(
        &self,
        _: *const c_void,
        _: Ref<IDWriteTextRenderer>,
        _: f32,
        _: f32,
        _: BOOL,
        _: BOOL,
        _: Ref<IUnknown>,
    ) -> Result<()> {
        // Slint draws the image inside the reserved space.
        Ok(())
    }

    fn GetMetrics(&self) -> Result<DWRITE_INLINE_OBJECT_METRICS> {
        Ok(DWRITE_INLINE_OBJECT_METRICS {
            width: self.width,
            height: self.height,
            baseline: self.height,
            supportsSideways: false.into(),
        })
    }

    fn GetOverhangMetrics(&self) -> Result<DWRITE_OVERHANG_METRICS> {
        Ok(DWRITE_OVERHANG_METRICS::default())
    }

    fn GetBreakConditions(
        &self,
        before: *mut DWRITE_BREAK_CONDITION,
        after: *mut DWRITE_BREAK_CONDITION,
    ) -> Result<()> {
        // Both pointers address writable out parameters in the DirectWrite call.
        unsafe {
            before.write(DWRITE_BREAK_CONDITION_NEUTRAL);
            after.write(DWRITE_BREAK_CONDITION_NEUTRAL);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format() -> Format<'static> {
        Format {
            family: "Segoe UI",
            size: 14.0,
            weight: 400,
            foreground: [24, 24, 24, 255],
            alignment: Alignment::Leading,
            wrap: true,
            rtl: false,
            paragraph_spacing: 0.0,
        }
    }

    #[test]
    fn flows_at_line_boundaries_across_regions_of_different_widths() {
        let renderer = Renderer::new().unwrap();
        let paragraph = Paragraph {
            text: "one two three four five six seven eight nine ten"
                .encode_utf16()
                .collect(),
            ..Default::default()
        };
        let length = paragraph.text.len() as u32;
        let regions = [
            Region {
                width: 90.0,
                height: 22.0,
            },
            Region {
                width: 132.0,
                height: 0.0,
            },
        ];
        let flow = renderer
            .render(&[paragraph], &regions, &format(), 1.0)
            .unwrap();
        let start_of_second = flow.frames[1].hits.first().unwrap().range.start;
        assert!(start_of_second > 0 && start_of_second < length);
        assert_eq!(
            flow.frames[0].hits.last().unwrap().range.end,
            start_of_second
        );
        assert_eq!(flow.consumed, length);
        assert!(!flow.has_overflow);
        assert!(
            flow.frames
                .iter()
                .all(|frame| frame.pixels.chunks_exact(4).any(|pixel| pixel[3] != 0))
        );
    }

    #[test]
    fn inline_objects_reserve_width_and_a_baseline_without_painting_a_glyph() {
        let renderer = Renderer::new().unwrap();
        let paragraph = Paragraph {
            text: "\u{fffc}".encode_utf16().collect(),
            spans: vec![Span {
                range: 0..1,
                inline_size: Some((24.0, 20.0)),
                ..Default::default()
            }],
        };
        let flow = renderer
            .render(
                &[paragraph],
                &[Region {
                    width: 100.0,
                    height: 0.0,
                }],
                &format(),
                1.25,
            )
            .unwrap();
        let frame = &flow.frames[0];
        assert_eq!(frame.pixel_width, 125);
        assert_eq!(frame.hits[0].rect.width, 24.0);
        assert!(frame.hits[0].baseline >= 20.0);
        assert!(frame.pixels.chunks_exact(4).all(|pixel| pixel[3] == 0));
    }
}

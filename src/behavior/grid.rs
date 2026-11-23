use std::ops::Range;

use slint::{Model, ModelRc, VecModel};

use crate::{
    GridBehavior, GridChild, GridDefinition, GridLayoutResult, GridLengthKind, GridMeasureRequest,
    GridOptions, GridTrackPlacement, ItemsRepeaterGeometry, ItemsRepeaterPlacement,
    ItemsRepeaterSize,
};

const EPSILON: f32 = 0.00001;
const MAX_STAR_WEIGHT: f32 = f32::MAX / u32::MAX as f32;

/// The host measures the named child against the proposed size without mutating the grid.
/// A negative available dimension is unconstrained, and a child may be measured twice.
pub fn install(
    behavior: GridBehavior<'_>,
    measure: impl Fn(GridMeasureRequest) -> ItemsRepeaterSize + 'static,
) {
    behavior.on_layout(move |columns, rows, children, options| {
        layout(columns, rows, children, options, &measure)
    });
}

fn read_model<T: Clone>(model: &ModelRc<T>) -> Vec<T> {
    model.model_tracker().track_row_count_changes();
    model
        .iter()
        .enumerate()
        .map(|(index, value)| {
            model.model_tracker().track_row_data_changes(index);
            value
        })
        .collect()
}

fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn even(value: f32) -> f32 {
    (value / 2.0 + 0.5).floor() * 2.0
}

fn limit(value: f32) -> f32 {
    if value.is_finite() && value >= 0.0 {
        value
    } else {
        f32::INFINITY
    }
}

fn fit(value: f32, limit: f32) -> f32 {
    finite(value).max(0.0).min(limit.max(0.0))
}

struct Track {
    kind: GridLengthKind,
    effective: GridLengthKind,
    weight: f32,
    minimum: f32,
    maximum: f32,
    allocation: f32,
}

impl Track {
    fn new(input: GridDefinition, unbounded: bool) -> Self {
        let mut minimum = even(finite(input.min_size).max(0.0));
        let maximum = if input.has_max {
            even(finite(input.max_size).max(0.0))
        } else {
            f32::INFINITY
        };
        let kind = input.kind;
        let mut allocation = maximum.max(minimum);
        if kind == GridLengthKind::Pixel {
            minimum = minimum.max(even(finite(input.pixels).max(0.0)).min(maximum));
            allocation = minimum;
        }
        let effective = if unbounded && kind == GridLengthKind::Star {
            GridLengthKind::Auto
        } else {
            kind
        };
        Self {
            kind,
            effective,
            weight: finite(input.weight).clamp(0.0, MAX_STAR_WEIGHT),
            minimum,
            maximum,
            allocation,
        }
    }

    fn measured(&self) -> f32 {
        if self.effective == GridLengthKind::Auto {
            self.minimum
        } else {
            self.allocation
        }
    }

    fn preferred(&self) -> f32 {
        self.minimum.max(self.measured())
    }

    fn ceiling(&self) -> f32 {
        self.maximum.max(self.minimum)
    }
}

struct Axis {
    tracks: Vec<Track>,
    spacing: f32,
    available: f32,
}

#[derive(Default, Clone, Copy)]
struct Units {
    auto: bool,
    star: bool,
}

impl Axis {
    fn new(mut definitions: Vec<GridDefinition>, spacing: f32, available: f32) -> Self {
        if definitions.is_empty() {
            definitions.push(GridDefinition {
                kind: GridLengthKind::Star,
                weight: 1.0,
                ..Default::default()
            });
        }
        let spacing = finite(spacing);
        let available = limit(available) - spacing * definitions.len().saturating_sub(1) as f32;
        let tracks = definitions
            .into_iter()
            .map(|input| Track::new(input, available.is_infinite()))
            .collect();
        Self {
            tracks,
            spacing,
            available,
        }
    }

    fn range(&self, index: i32, span: i32) -> Range<usize> {
        let first = (index.max(0) as usize).min(self.tracks.len() - 1);
        first..first + (span.max(1) as usize).min(self.tracks.len() - first)
    }

    fn units(&self, range: &Range<usize>) -> Units {
        self.tracks[range.clone()]
            .iter()
            .fold(Units::default(), |flags, track| Units {
                auto: flags.auto || track.effective == GridLengthKind::Auto,
                star: flags.star || track.effective == GridLengthKind::Star,
            })
    }

    fn measure_limit(&self, range: &Range<usize>, flags: Units) -> f32 {
        if flags.auto && !flags.star {
            f32::INFINITY
        } else {
            self.tracks[range.clone()]
                .iter()
                .rev()
                .map(Track::measured)
                .sum::<f32>()
                + self.spacing * range.len().saturating_sub(1) as f32
        }
    }

    fn desired(&self) -> f32 {
        (self.tracks.iter().map(|track| track.minimum).sum::<f32>() + self.gaps()).max(0.0)
    }

    fn gaps(&self) -> f32 {
        self.spacing * self.tracks.len().saturating_sub(1) as f32
    }

    fn resolve_stars(&mut self) {
        if self.available.is_finite() {
            self.distribute_stars(self.available, false);
        }
    }

    fn arrange(&mut self, available: f32) -> Vec<GridTrackPlacement> {
        let available = if limit(available).is_finite() {
            available
        } else {
            self.desired()
        };
        let space = available - self.gaps();
        let used = self.distribute_stars(space, true);
        if used > space + EPSILON {
            let mut indices: Vec<_> = (0..self.tracks.len()).collect();
            indices.sort_by(|&a, &b| {
                let excess = |i: usize| self.tracks[i].allocation - self.tracks[i].minimum;
                excess(a).total_cmp(&excess(b))
            });
            let mut excess = used - space;
            for (position, index) in indices.into_iter().enumerate() {
                let remaining = (self.tracks.len() - position) as f32;
                let track = &mut self.tracks[index];
                let shrink = (excess / remaining).min(track.allocation - track.minimum);
                track.allocation -= shrink;
                excess -= shrink;
            }
        }

        let mut offset = 0.0;
        self.tracks
            .iter()
            .map(|track| {
                let start = even(offset);
                let end = offset + track.allocation;
                offset = end + self.spacing;
                GridTrackPlacement {
                    offset: start,
                    size: (even(end) - start).max(0.0),
                }
            })
            .collect()
    }

    fn distribute_stars(&mut self, space: f32, final_pass: bool) -> f32 {
        let mut stars = Vec::new();
        let mut fixed = 0.0;
        for (index, track) in self.tracks.iter_mut().enumerate() {
            let kind = if final_pass {
                track.kind
            } else {
                track.effective
            };
            if kind == GridLengthKind::Star {
                stars.push(index);
            } else {
                if final_pass {
                    track.allocation = track.minimum;
                }
                fixed += if final_pass {
                    track.allocation
                } else {
                    track.measured()
                };
            }
        }
        let weight = |track: &Track| {
            if track.weight < EPSILON {
                0.0
            } else {
                track.weight
            }
        };
        stars.sort_by(|&a, &b| {
            let cap_per_weight = |i: usize| {
                let track = &self.tracks[i];
                let weight = weight(track);
                if weight == 0.0 {
                    0.0
                } else {
                    track.ceiling().min(MAX_STAR_WEIGHT) / weight
                }
            };
            cap_per_weight(a).total_cmp(&cap_per_weight(b))
        });
        let mut weights = vec![0.0_f64; stars.len() + 1];
        for position in (0..stars.len()).rev() {
            weights[position] =
                weights[position + 1] + f64::from(weight(&self.tracks[stars[position]]));
        }
        let mut used = fixed;
        for (position, index) in stars.into_iter().enumerate() {
            let track = &mut self.tracks[index];
            let weight = weight(track);
            track.allocation = if weight == 0.0 {
                track.minimum
            } else {
                let fraction = (f64::from(weight) / weights[position]) as f32;
                ((space - used).max(0.0) * fraction)
                    .min(track.maximum)
                    .max(track.minimum)
            };
            used += track.allocation;
        }
        used
    }

    fn apply_span(&mut self, span: SpanRequest) {
        let count = span.range.len();
        let requested = (span.desired - self.spacing * (count - 1) as f32).max(0.0);
        let tracks = &mut self.tracks[span.range];
        let minimum: f32 = tracks.iter().map(|track| track.minimum).sum();
        if requested <= minimum || requested <= EPSILON {
            return;
        }
        let preferred: f32 = tracks.iter().map(Track::preferred).sum();
        let maximum: f32 = tracks.iter().map(Track::ceiling).sum();
        let mut indices: Vec<_> = (0..count).collect();
        if requested <= preferred {
            indices.retain(|&index| tracks[index].kind != GridLengthKind::Auto);
            indices.sort_by(|&a, &b| tracks[a].preferred().total_cmp(&tracks[b].preferred()));
            let mut remaining = requested
                - tracks
                    .iter()
                    .filter(|track| track.kind == GridLengthKind::Auto)
                    .map(|track| track.minimum)
                    .sum::<f32>();
            let active = indices.len();
            for (position, index) in indices.into_iter().enumerate() {
                let track = &mut tracks[index];
                let value = (remaining / (active - position) as f32).min(track.preferred());
                track.minimum = track.minimum.max(value);
                remaining -= value;
                if remaining <= EPSILON {
                    break;
                }
            }
        } else if requested <= maximum {
            indices.sort_by(|&a, &b| {
                let auto = |i: usize| tracks[i].kind == GridLengthKind::Auto;
                auto(a)
                    .cmp(&auto(b))
                    .then_with(|| tracks[a].ceiling().total_cmp(&tracks[b].ceiling()))
            });
            let non_auto = tracks
                .iter()
                .filter(|track| track.kind != GridLengthKind::Auto)
                .count();
            let mut remaining = requested - preferred;
            for (position, index) in indices.into_iter().enumerate() {
                let active = if position < non_auto { non_auto } else { count };
                let track = &mut tracks[index];
                let preferred = track.preferred();
                track.minimum =
                    (preferred + remaining / (active - position) as f32).min(track.ceiling());
                remaining -= track.minimum - preferred;
                if remaining <= EPSILON {
                    break;
                }
            }
        } else {
            let equal = requested / count as f32;
            let largest = tracks.iter().map(Track::ceiling).fold(0.0, f32::max);
            let capacity = largest * count as f32 - maximum;
            for track in tracks {
                let ceiling = track.ceiling();
                let value = if equal < largest && largest - equal > EPSILON {
                    ceiling + (largest - ceiling) * (requested - maximum) / capacity
                } else {
                    equal
                };
                track.minimum = track.minimum.max(value);
            }
        }
    }
}

struct Cell {
    columns: Range<usize>,
    rows: Range<usize>,
    horizontal: Units,
    vertical: Units,
}

impl Cell {
    fn group(&self) -> usize {
        match (
            self.vertical.star,
            self.horizontal.star,
            self.horizontal.auto,
        ) {
            (false, false, _) => 0,
            (true, false, true) => 1,
            (false, true, _) => 2,
            _ => 3,
        }
    }
}

struct SpanRequest {
    range: Range<usize>,
    desired: f32,
}

fn accumulate(
    tracks: &mut [Track],
    spans: &mut Vec<SpanRequest>,
    range: &Range<usize>,
    desired: f32,
) {
    if range.len() == 1 {
        tracks[range.start].minimum = tracks[range.start].minimum.max(desired);
    } else if let Some(span) = spans.iter_mut().find(|span| span.range == *range) {
        span.desired = span.desired.max(desired);
    } else {
        spans.push(SpanRequest {
            range: range.clone(),
            desired,
        });
    }
}

fn measure_child(
    child: &GridChild,
    width: f32,
    height: f32,
    measure: &impl Fn(GridMeasureRequest) -> ItemsRepeaterSize,
) -> ItemsRepeaterSize {
    if child.collapsed {
        return ItemsRepeaterSize::default();
    }
    let proposed = |limit: f32| {
        if limit.is_infinite() {
            -1.0
        } else {
            limit.max(0.0)
        }
    };
    let content = measure(GridMeasureRequest {
        key: child.key.clone(),
        preferred: child.size.clone(),
        available: ItemsRepeaterSize {
            width: proposed(width),
            height: proposed(height),
        },
    });
    ItemsRepeaterSize {
        width: fit(content.width, width),
        height: fit(content.height, height),
    }
}

struct GridSizing {
    columns: Axis,
    rows: Axis,
}

impl GridSizing {
    fn measure_group(
        &mut self,
        group: &[usize],
        cells: &[Cell],
        children: &[GridChild],
        force_unbounded_rows: bool,
        ignore_width: bool,
        measure: &impl Fn(GridMeasureRequest) -> ItemsRepeaterSize,
    ) {
        let mut column_spans = Vec::new();
        let mut row_spans = Vec::new();
        for &index in group {
            let cell = &cells[index];
            let width = self.columns.measure_limit(&cell.columns, cell.horizontal);
            let height = if force_unbounded_rows {
                f32::INFINITY
            } else {
                self.rows.measure_limit(&cell.rows, cell.vertical)
            };
            let desired = measure_child(&children[index], width, height, measure);
            if !ignore_width {
                accumulate(
                    &mut self.columns.tracks,
                    &mut column_spans,
                    &cell.columns,
                    desired.width,
                );
            }
            if !force_unbounded_rows {
                accumulate(
                    &mut self.rows.tracks,
                    &mut row_spans,
                    &cell.rows,
                    desired.height,
                );
            }
        }
        for span in column_spans {
            self.columns.apply_span(span);
        }
        for span in row_spans {
            self.rows.apply_span(span);
        }
    }

    fn measure(
        &mut self,
        children: &[GridChild],
        measure: &impl Fn(GridMeasureRequest) -> ItemsRepeaterSize,
    ) -> Vec<Cell> {
        let cells: Vec<_> = children
            .iter()
            .map(|child| {
                let columns = self.columns.range(child.column, child.column_span);
                let rows = self.rows.range(child.row, child.row_span);
                let horizontal = self.columns.units(&columns);
                let vertical = self.rows.units(&rows);
                Cell {
                    columns,
                    rows,
                    horizontal,
                    vertical,
                }
            })
            .collect();
        let mut groups: [Vec<usize>; 4] = Default::default();
        for (index, cell) in cells.iter().enumerate() {
            groups[cell.group()].push(index);
        }
        let row_auto_depends_on_star = groups[2].iter().any(|&index| cells[index].vertical.auto);
        self.measure_group(&groups[0], &cells, children, false, false, measure);
        if row_auto_depends_on_star {
            // First determine Auto columns without constraining their children's Star rows.
            self.measure_group(&groups[1], &cells, children, true, false, measure);
            self.columns.resolve_stars();
            self.measure_group(&groups[2], &cells, children, false, false, measure);
            self.rows.resolve_stars();
            self.measure_group(&groups[1], &cells, children, false, true, measure);
        } else {
            self.rows.resolve_stars();
            self.measure_group(&groups[1], &cells, children, false, false, measure);
            self.columns.resolve_stars();
            self.measure_group(&groups[2], &cells, children, false, false, measure);
        }
        self.measure_group(&groups[3], &cells, children, false, false, measure);
        cells
    }
}

fn slot(tracks: &[GridTrackPlacement], range: Range<usize>) -> (f32, f32) {
    let first = tracks[range.start].offset;
    let last = &tracks[range.end - 1];
    (first, (last.offset + last.size - first).max(0.0))
}

fn finalize(
    children: &[GridChild],
    desired: ItemsRepeaterSize,
    columns: Vec<GridTrackPlacement>,
    rows: Vec<GridTrackPlacement>,
    mirror: Option<f32>,
    mut bounds: impl FnMut(usize) -> (f32, f32, f32, f32),
) -> GridLayoutResult {
    let placements: Vec<_> = children
        .iter()
        .enumerate()
        .map(|(index, child)| {
            let (x, y, width, height) = bounds(index);
            let mut item = ItemsRepeaterPlacement {
                index: index as i32,
                key: child.key.clone(),
                ..Default::default()
            };
            if !child.collapsed {
                item.x = mirror.map_or(x, |edge| edge - x - width);
                item.y = y;
                item.width = width;
                item.height = height;
            }
            item
        })
        .collect();
    GridLayoutResult {
        geometry: ItemsRepeaterGeometry {
            placements: ModelRc::new(VecModel::from(placements)),
            width: even(desired.width),
            height: even(desired.height),
        },
        columns: ModelRc::new(VecModel::from(columns)),
        rows: ModelRc::new(VecModel::from(rows)),
    }
}

fn arrange_overlay(
    children: &[GridChild],
    options: GridOptions,
    measure: &impl Fn(GridMeasureRequest) -> ItemsRepeaterSize,
) -> GridLayoutResult {
    let mut desired = ItemsRepeaterSize::default();
    let measurements: Vec<_> = children
        .iter()
        .map(|child| {
            let size = measure_child(
                child,
                limit(options.measure.width),
                limit(options.measure.height),
                measure,
            );
            desired.width = desired.width.max(size.width);
            desired.height = desired.height.max(size.height);
            size
        })
        .collect();
    let initial = |available: f32, natural: f32| {
        if limit(available).is_finite() {
            available
        } else {
            natural
        }
    };
    let mut width = initial(options.arrange.width, desired.width);
    let mut height = initial(options.arrange.height, desired.height);
    let mirror = options.rtl.then_some(even(width));
    let columns = vec![GridTrackPlacement {
        offset: 0.0,
        size: even(width.max(desired.width)),
    }];
    let rows = vec![GridTrackPlacement {
        offset: 0.0,
        size: even(height.max(desired.height)),
    }];
    finalize(children, desired, columns, rows, mirror, |index| {
        width = width.max(measurements[index].width);
        height = height.max(measurements[index].height);
        (0.0, 0.0, even(width), even(height))
    })
}

fn layout(
    columns: ModelRc<GridDefinition>,
    rows: ModelRc<GridDefinition>,
    children: ModelRc<GridChild>,
    options: GridOptions,
    measure: &impl Fn(GridMeasureRequest) -> ItemsRepeaterSize,
) -> GridLayoutResult {
    let columns = read_model(&columns);
    let rows = read_model(&rows);
    let children = read_model(&children);
    if columns.is_empty() && rows.is_empty() {
        return arrange_overlay(&children, options, measure);
    }

    let mut grid = GridSizing {
        columns: Axis::new(columns, options.column_spacing, options.measure.width),
        rows: Axis::new(rows, options.row_spacing, options.measure.height),
    };
    let cells = grid.measure(&children, measure);
    let desired = ItemsRepeaterSize {
        width: grid.columns.desired(),
        height: grid.rows.desired(),
    };
    let columns = grid.columns.arrange(options.arrange.width);
    let rows = grid.rows.arrange(options.arrange.height);
    let mirror = options.rtl.then(|| {
        even(if limit(options.arrange.width).is_finite() {
            options.arrange.width
        } else {
            desired.width
        })
    });
    let bounds: Vec<_> = cells
        .into_iter()
        .map(|cell| {
            let (x, width) = slot(&columns, cell.columns);
            let (y, height) = slot(&rows, cell.rows);
            (x, y, width, height)
        })
        .collect();
    finalize(&children, desired, columns, rows, mirror, |index| {
        bounds[index]
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    fn model<T: Clone + 'static>(values: Vec<T>) -> ModelRc<T> {
        ModelRc::new(VecModel::from(values))
    }

    fn auto() -> GridDefinition {
        GridDefinition {
            kind: GridLengthKind::Auto,
            ..Default::default()
        }
    }

    fn pixels(pixels: f32) -> GridDefinition {
        GridDefinition {
            kind: GridLengthKind::Pixel,
            pixels,
            ..Default::default()
        }
    }

    fn star(weight: f32) -> GridDefinition {
        GridDefinition {
            kind: GridLengthKind::Star,
            weight,
            ..Default::default()
        }
    }

    fn size(width: f32, height: f32) -> ItemsRepeaterSize {
        ItemsRepeaterSize { width, height }
    }

    fn child(key: &str, row: i32, column: i32, width: f32, height: f32) -> GridChild {
        GridChild {
            key: key.into(),
            row,
            column,
            size: size(width, height),
            ..Default::default()
        }
    }

    fn tracks(values: &ModelRc<GridTrackPlacement>) -> Vec<f32> {
        values.iter().map(|track| track.size).collect()
    }

    #[test]
    fn star_ceiling_redistributes_space_and_minima_can_overflow_the_viewport() {
        let mut capped = star(1.0);
        capped.min_size = 40.0;
        capped.max_size = 50.0;
        capped.has_max = true;
        let mut large = star(2.0);
        large.min_size = 40.0;
        let columns = model(vec![pixels(60.0), auto(), capped, large]);
        let items = model(vec![child("auto", 0, 1, 70.0, 32.0)]);
        for (width, expected) in [
            (310.0, vec![60.0, 70.0, 50.0, 130.0]),
            (180.0, vec![60.0, 70.0, 40.0, 40.0]),
        ] {
            let actual = layout(
                columns.clone(),
                model(vec![pixels(44.0)]),
                items.clone(),
                GridOptions {
                    measure: size(width, 44.0),
                    arrange: size(width, 44.0),
                    ..Default::default()
                },
                &|request| request.preferred,
            );
            assert_eq!(tracks(&actual.columns), expected);
        }
    }

    #[test]
    fn linked_auto_and_star_tracks_measure_the_dependent_child_twice() {
        let calls = RefCell::new(Vec::new());
        let result = layout(
            model(vec![auto(), star(1.0)]),
            model(vec![auto(), star(1.0)]),
            model(vec![
                child("auto column", 1, 0, 72.0, 62.0),
                child("auto row", 0, 1, 40.0, 30.0),
            ]),
            GridOptions {
                measure: size(240.0, 180.0),
                arrange: size(240.0, 180.0),
                row_spacing: 8.0,
                column_spacing: 8.0,
                ..Default::default()
            },
            &|request| {
                calls.borrow_mut().push((
                    request.key.clone(),
                    request.available.width,
                    request.available.height,
                ));
                match request.key.as_str() {
                    "auto row" if request.available.width == 160.0 => size(40.0, 50.0),
                    "auto column" if request.available.height >= 0.0 => size(114.0, 62.0),
                    _ => request.preferred,
                }
            },
        );
        assert_eq!(tracks(&result.columns), [72.0, 160.0]);
        assert_eq!(tracks(&result.rows), [50.0, 122.0]);
        let calls = calls.into_inner();
        assert_eq!(
            calls
                .iter()
                .map(|(key, w, h)| (key.as_str(), *w, *h))
                .collect::<Vec<_>>(),
            [
                ("auto column", -1.0, -1.0),
                ("auto row", 160.0, -1.0),
                ("auto column", -1.0, 122.0)
            ]
        );
    }

    #[test]
    fn spans_include_the_gutter_and_distribute_an_overconstrained_auto_span() {
        let capped = |maximum| GridDefinition {
            max_size: maximum,
            has_max: true,
            ..auto()
        };
        let mut spanning = child("span", 0, 0, 158.0, 32.0);
        spanning.column_span = 3;
        let result = layout(
            model(vec![capped(40.0), capped(80.0)]),
            model(vec![pixels(40.0)]),
            model(vec![spanning]),
            GridOptions {
                measure: size(200.0, 40.0),
                arrange: size(200.0, 40.0),
                column_spacing: 8.0,
                rtl: true,
                ..Default::default()
            },
            &|request| request.preferred,
        );
        assert_eq!(tracks(&result.columns), [70.0, 80.0]);
        let cell = result.geometry.placements.row_data(0).unwrap();
        assert_eq!((cell.x, cell.width), (42.0, 158.0));
    }

    #[test]
    fn a_fixed_row_span_can_grow_the_pixel_column_before_a_star_row_measures_auto() {
        let mut spanning = child("footer", 1, 0, 144.0, 36.0);
        spanning.column_span = 2;
        let result = layout(
            model(vec![pixels(64.0), auto()]),
            model(vec![star(1.0), pixels(36.0)]),
            model(vec![child("auto", 0, 1, 64.0, 44.0), spanning]),
            GridOptions {
                measure: size(160.0, 160.0),
                arrange: size(160.0, 160.0),
                column_spacing: 8.0,
                ..Default::default()
            },
            &|request| request.preferred,
        );
        assert_eq!(tracks(&result.columns), [136.0, 64.0]);
        assert_eq!(result.geometry.width, 208.0);
    }

    #[test]
    fn unbounded_stars_measure_like_auto_then_use_weights_during_arrange() {
        let result = layout(
            model(vec![star(1.0), star(2.0)]),
            model(Vec::new()),
            model(vec![
                child("left", 0, 0, 40.0, 24.0),
                child("right", 0, 1, 20.0, 30.0),
            ]),
            GridOptions {
                measure: size(-1.0, -1.0),
                arrange: size(150.0, 60.0),
                ..Default::default()
            },
            &|request| {
                assert_eq!(
                    (request.available.width, request.available.height),
                    (-1.0, -1.0)
                );
                request.preferred
            },
        );
        assert_eq!(
            (result.geometry.width, result.geometry.height),
            (60.0, 30.0)
        );
        assert_eq!(tracks(&result.columns), [50.0, 100.0]);
        assert_eq!(tracks(&result.rows), [60.0]);
    }

    #[test]
    fn grid_with_no_definitions_overlays_children_and_keeps_collapsed_source_indices() {
        let mut hidden = child("hidden", 0, 0, 200.0, 200.0);
        hidden.collapsed = true;
        let result = layout(
            model(Vec::new()),
            model(Vec::new()),
            model(vec![
                child("one", 1, 1, 64.0, 30.0),
                hidden,
                child("two", 3, 3, 120.0, 60.0),
            ]),
            GridOptions {
                measure: size(-1.0, -1.0),
                arrange: size(40.0, 20.0),
                ..Default::default()
            },
            &|request| request.preferred,
        );
        let items: Vec<_> = result
            .geometry
            .placements
            .iter()
            .map(|item| (item.index, item.x, item.y, item.width, item.height))
            .collect();
        assert_eq!(
            items,
            [
                (0, 0.0, 0.0, 64.0, 30.0),
                (1, 0.0, 0.0, 0.0, 0.0),
                (2, 0.0, 0.0, 120.0, 60.0)
            ]
        );
    }
}

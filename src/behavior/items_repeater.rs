use std::{collections::HashSet, ops::Range};

use slint::{Model, ModelRc, ModelTracker, SharedString, VecModel, language::Orientation};

use crate::{
    ItemsRepeaterBehavior, ItemsRepeaterGeometry, ItemsRepeaterJustification, ItemsRepeaterLayout,
    ItemsRepeaterLayoutOptions, ItemsRepeaterPlacement, ItemsRepeaterSize, ItemsRepeaterStretch,
    ItemsRepeaterWindow,
};

pub fn install(behavior: ItemsRepeaterBehavior<'_>) {
    behavior.on_arrange(arrange);
    behavior.on_visible(visible);
    behavior.on_update(update);
    behavior.set_ready(true);
}

#[derive(Clone, Copy)]
struct Axes {
    main_is_x: bool,
}

impl Axes {
    fn split(self, x: f32, y: f32) -> (f32, f32) {
        if self.main_is_x { (x, y) } else { (y, x) }
    }

    fn place(
        self,
        index: usize,
        main: f32,
        cross: f32,
        length: f32,
        breadth: f32,
    ) -> ItemsRepeaterPlacement {
        let (x, y) = self.split(main, cross);
        let (width, height) = self.split(length, breadth);
        ItemsRepeaterPlacement {
            index: index as i32,
            key: SharedString::default(),
            x,
            y,
            width,
            height,
        }
    }
}

struct Line {
    rows: Range<usize>,
    start: f32,
    end: f32,
}

struct GeometryModel {
    placements: Vec<ItemsRepeaterPlacement>,
    lines: Vec<Line>,
    axes: Axes,
    rtl: bool,
}

impl Model for GeometryModel {
    type Data = ItemsRepeaterPlacement;

    fn row_count(&self) -> usize {
        self.placements.len()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.placements.get(row).cloned()
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn nonnegative(n: f32) -> f32 {
    if n.is_finite() { n.max(0.0) } else { 0.0 }
}

fn even(n: f32) -> f32 {
    (n / 2.0).round() * 2.0
}

fn item_size(
    sizes: &dyn Model<Data = ItemsRepeaterSize>,
    row: usize,
    fallback: &ItemsRepeaterSize,
    axes: Axes,
) -> (f32, f32) {
    sizes.model_tracker().track_row_data_changes(row);
    let size = sizes.row_data(row).unwrap_or_else(|| fallback.clone());
    axes.split(
        if size.width > 0.0 {
            nonnegative(size.width)
        } else {
            nonnegative(fallback.width)
        }
        .max(2.0),
        if size.height > 0.0 {
            nonnegative(size.height)
        } else {
            nonnegative(fallback.height)
        }
        .max(2.0),
    )
}

fn arrange(
    count: i32,
    keys: ModelRc<SharedString>,
    sizes: ModelRc<ItemsRepeaterSize>,
    custom: ModelRc<ItemsRepeaterPlacement>,
    options: ItemsRepeaterLayoutOptions,
) -> ItemsRepeaterGeometry {
    let count = count.max(0) as usize;
    let stack = options.kind == ItemsRepeaterLayout::Stack;
    let uniform = options.kind == ItemsRepeaterLayout::UniformGrid;
    let lined = options.kind == ItemsRepeaterLayout::LinedFlow;
    let axes = Axes {
        main_is_x: !lined && stack == (options.orientation == Orientation::Horizontal),
    };
    let mut model = GeometryModel {
        placements: Vec::with_capacity(count),
        lines: Vec::new(),
        axes,
        rtl: options.rtl,
    };
    let (viewport_width, viewport_height) = (
        nonnegative(options.viewport.width),
        nonnegative(options.viewport.height),
    );
    let (_, cross_viewport) = axes.split(viewport_width, viewport_height);
    let (main_gap, cross_gap) = axes.split(
        nonnegative(options.spacing.width),
        nonnegative(options.spacing.height),
    );

    if options.kind == ItemsRepeaterLayout::Custom {
        custom.model_tracker().track_row_count_changes();
        for index in 0..count {
            custom.model_tracker().track_row_data_changes(index);
            let mut placement = custom.row_data(index).unwrap_or_default();
            placement.index = index as i32;
            placement.x = nonnegative(placement.x);
            placement.y = nonnegative(placement.y);
            placement.width = nonnegative(placement.width);
            placement.height = nonnegative(placement.height);
            model.placements.push(placement);
        }
    } else if lined {
        arrange_lined(
            &mut model,
            &sizes,
            &options,
            count,
            viewport_width,
            cross_gap,
            main_gap,
        );
    } else if count > 0 {
        sizes.model_tracker().track_row_count_changes();
        let mut limit = if options.max_items_per_line > 0 {
            options.max_items_per_line as usize
        } else {
            count
        };
        let (mut cell_main, mut cell_cross) = item_size(&sizes, 0, &options.item_size, axes);
        if stack {
            limit = 1;
        } else if uniform && cross_viewport > 0.0 {
            // WinUI fits whole cells including their spacing, then distributes the spare pixels.
            limit =
                limit.min(((cross_viewport / (cell_cross + cross_gap)).floor() as usize).max(1));
            if options.stretch != ItemsRepeaterStretch::None {
                let available_per_cell = (cross_viewport + cross_gap) / limit as f32 - cross_gap;
                let cross = even(available_per_cell).max(2.0);
                if options.stretch == ItemsRepeaterStretch::Uniform {
                    cell_main = even(cell_main * cross / cell_cross).max(2.0);
                }
                cell_cross = cross;
            }
        }

        let mut main = 0.0;
        let mut first = 0;
        while first < count {
            let mut length: f32 = 0.0;
            let mut cross: f32 = 0.0;
            for index in first..count.min(first + limit) {
                let (item_main, item_cross) = if uniform {
                    (cell_main, cell_cross)
                } else {
                    item_size(&sizes, index, &options.item_size, axes)
                };
                if index > first
                    && !uniform
                    && cross_viewport > 0.0
                    && cross + item_cross > cross_viewport
                {
                    break;
                }
                model
                    .placements
                    .push(axes.place(index, main, cross, item_main, item_cross));
                cross += item_cross + cross_gap;
                length = length.max(item_main);
            }
            let end = model.placements.len();
            if !stack {
                let spare = (cross_viewport - cross + cross_gap).max(0.0);
                justify(
                    &mut model.placements[first..end],
                    axes,
                    spare,
                    options.justification,
                );
            }
            model.lines.push(Line {
                rows: first..end,
                start: main,
                end: main + length,
            });
            main += length + main_gap;
            first = end;
        }
    }

    let (mut width, mut height) = (0.0_f32, 0.0_f32);
    keys.model_tracker().track_row_count_changes();
    let key_count = keys.row_count();
    for (index, rect) in model.placements.iter_mut().enumerate() {
        if index < key_count {
            keys.model_tracker().track_row_data_changes(index);
            rect.key = keys.row_data(index).unwrap_or_default();
        }
        if rect.width > 0.0 && rect.height > 0.0 {
            width = width.max(rect.x + rect.width);
            height = height.max(rect.y + rect.height);
        }
    }
    if options.rtl {
        width = width.max(viewport_width);
        for rect in &mut model.placements {
            rect.x = width - rect.x - rect.width;
        }
    }
    ItemsRepeaterGeometry {
        placements: ModelRc::new(model),
        width,
        height,
    }
}

fn justify(
    items: &mut [ItemsRepeaterPlacement],
    axes: Axes,
    spare: f32,
    alignment: ItemsRepeaterJustification,
) {
    let count = items.len() as f32;
    let (start, gap) = match alignment {
        ItemsRepeaterJustification::Start => return,
        ItemsRepeaterJustification::Center => (spare / 2.0, 0.0),
        ItemsRepeaterJustification::End => (spare, 0.0),
        ItemsRepeaterJustification::SpaceAround => (spare / count / 2.0, spare / count),
        ItemsRepeaterJustification::SpaceBetween => (0.0, spare / (count - 1.0).max(1.0)),
        ItemsRepeaterJustification::SpaceEvenly => (spare / (count + 1.0), spare / (count + 1.0)),
    };
    for (slot, item) in items.iter_mut().enumerate() {
        let shift = even(start + slot as f32 * gap);
        if axes.main_is_x {
            item.y += shift;
        } else {
            item.x += shift;
        }
    }
}

fn arrange_lined(
    model: &mut GeometryModel,
    sizes: &ModelRc<ItemsRepeaterSize>,
    options: &ItemsRepeaterLayoutOptions,
    count: usize,
    viewport: f32,
    item_gap: f32,
    line_gap: f32,
) {
    let height = even(nonnegative(options.item_size.height)).max(2.0);
    let minimum = even(nonnegative(options.item_size.width)).max(2.0);
    sizes.model_tracker().track_row_count_changes();
    let widths = (0..count)
        .map(|index| {
            sizes.model_tracker().track_row_data_changes(index);
            let Some(size) = sizes.row_data(index) else {
                return minimum;
            };
            let ratio = if size.height > 0.0 {
                size.width / size.height
            } else {
                1.0
            };
            even(nonnegative(ratio) * height).max(minimum)
        })
        .collect::<Vec<_>>();
    let mut first = 0;
    let mut y = 0.0;
    while first < count {
        let mut end = first + 1;
        let mut natural_width = widths[first];
        while let Some(&next) = widths.get(end) {
            let proposed = natural_width + item_gap + next;
            let min_width = (end - first + 1) as f32 * minimum + (end - first) as f32 * item_gap;
            if viewport > 0.0
                && (min_width > viewport
                    || proposed > viewport
                        && (options.stretch == ItemsRepeaterStretch::None
                            || proposed - viewport > viewport - natural_width))
            {
                break;
            }
            natural_width = proposed;
            end += 1;
        }
        let fill = options.stretch != ItemsRepeaterStretch::None
            && viewport >= minimum
            && (end < count || natural_width > viewport);
        let available = if fill {
            viewport - (end - first - 1) as f32 * item_gap
        } else {
            natural_width - (end - first - 1) as f32 * item_gap
        };
        let weight = widths[first..end]
            .iter()
            .map(|width| width - minimum)
            .sum::<f32>();
        let spare = (available - (end - first) as f32 * minimum).max(0.0);
        let mut x = 0.0;
        let mut distributed = 0.0;
        let mut cumulative = 0.0;
        for (offset, &natural) in widths[first..end].iter().enumerate() {
            let width = if fill {
                cumulative += if weight > 0.0 {
                    (natural - minimum) / weight
                } else {
                    1.0 / (end - first) as f32
                };
                let edge = even(cumulative * spare);
                let width = minimum + edge - distributed;
                distributed = edge;
                width
            } else {
                natural
            };
            model
                .placements
                .push(model.axes.place(first + offset, y, x, height, width));
            x += width + item_gap;
        }
        justify(
            &mut model.placements[first..end],
            model.axes,
            (viewport - x + item_gap).max(0.0),
            options.justification,
        );
        model.lines.push(Line {
            rows: first..end,
            start: y,
            end: y + height,
        });
        y += height + line_gap;
        first = end;
    }
}

fn visible(
    geometry: ItemsRepeaterGeometry,
    window: ItemsRepeaterWindow,
) -> ModelRc<ItemsRepeaterPlacement> {
    let Some(model) = geometry.placements.as_any().downcast_ref::<GeometryModel>() else {
        return ModelRc::default();
    };
    let mut rows = 0..model.placements.len();
    let window_range = |offset: f32, extent: f32, cache: f32| {
        if !window.virtualizing || extent <= 0.0 {
            return f32::NEG_INFINITY..f32::INFINITY;
        }
        let buffer = extent * nonnegative(cache) / 2.0;
        offset - buffer..offset + extent + buffer
    };
    let x = window_range(window.x, window.width, window.horizontal_cache_length);
    let y = window_range(window.y, window.height, window.vertical_cache_length);

    if !model.lines.is_empty() {
        let (start, end) = if !model.axes.main_is_x {
            (y.start, y.end)
        } else if model.rtl {
            (geometry.width - x.end, geometry.width - x.start)
        } else {
            (x.start, x.end)
        };
        let first = model.lines.partition_point(|line| line.end <= start);
        let last = model.lines.partition_point(|line| line.start < end);
        if first >= last {
            return ModelRc::default();
        }
        rows = model.lines[first].rows.start..model.lines[last - 1].rows.end;
    }
    let visible = model.placements[rows]
        .iter()
        .filter(|rect| {
            rect.width > 0.0
                && rect.height > 0.0
                && rect.x < x.end
                && x.start < rect.x + rect.width
                && rect.y < y.end
                && y.start < rect.y + rect.height
        })
        .cloned()
        .collect::<Vec<_>>();
    ModelRc::new(VecModel::from(visible))
}

fn identity(rect: &ItemsRepeaterPlacement) -> (i32, &str) {
    if rect.key.is_empty() {
        (rect.index, "")
    } else {
        (-1, rect.key.as_str())
    }
}

fn update(
    current: ModelRc<ItemsRepeaterPlacement>,
    next: ModelRc<ItemsRepeaterPlacement>,
) -> ModelRc<ItemsRepeaterPlacement> {
    let Some(rows) = current
        .as_any()
        .downcast_ref::<VecModel<ItemsRepeaterPlacement>>()
    else {
        return ModelRc::new(VecModel::from(next.iter().collect::<Vec<_>>()));
    };
    // Keep delegates whose keys overlap and remain in the same relative order.
    let desired = next.iter().collect::<Vec<_>>();
    let retained = desired.iter().map(identity).collect::<HashSet<_>>();
    for i in (0..rows.row_count()).rev() {
        if rows
            .row_data(i)
            .is_some_and(|row| !retained.contains(&identity(&row)))
        {
            rows.remove(i);
        }
    }
    for (slot, wanted) in desired.into_iter().enumerate() {
        match rows.row_data(slot) {
            Some(existing) if identity(&existing) == identity(&wanted) => {
                if existing != wanted {
                    let _ = rows.set_row_data(slot, wanted);
                }
            }
            _ => rows.insert(slot, wanted),
        }
    }
    while rows.row_count() > next.row_count() {
        rows.remove(rows.row_count() - 1);
    }
    current
}

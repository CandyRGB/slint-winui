use std::path::PathBuf;

use serde_json::Value;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

slint::slint! {
    import { SemanticZoomExample, SemanticZoomSampleBehavior, SemanticZoomSampleGroup,
        SemanticZoomSampleItem, SemanticZoomSampleCell, SemanticZoomSampleLayout } from "ui/semantic_zoom.slint";
    export { SemanticZoomExample, SemanticZoomSampleBehavior, SemanticZoomSampleGroup,
        SemanticZoomSampleItem, SemanticZoomSampleCell, SemanticZoomSampleLayout }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = SemanticZoomExample::new()?;
    let behavior = ui.global::<SemanticZoomSampleBehavior>();
    behavior.on_arrange(arrange);
    behavior.on_find_cell(|cells, key| {
        cells
            .iter()
            .position(|cell| cell.key == key)
            .map_or(-1, |index| index as i32)
    });
    behavior.on_find_group(|groups, key| {
        groups
            .iter()
            .position(|group| group.key == key)
            .map_or(-1, |index| index as i32)
    });
    behavior.on_hit_cell(|cells, x, y| {
        cells
            .iter()
            .position(|cell| {
                x >= cell.x && x < cell.x + cell.width && y >= cell.y && y < cell.y + cell.height
            })
            .map_or(-1, |index| index as i32)
    });
    behavior.on_move_detail(move_detail);
    ui.set_groups(ModelRc::new(VecModel::from(
        gallery_groups().unwrap_or_else(fallback_groups),
    )));
    ui.run()
}

fn gallery_groups() -> Option<Vec<SemanticZoomSampleGroup>> {
    let source = std::env::var_os("SLINT_WINUI_GALLERY_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("demo-assets/ControlInfoData.json")
        });
    let bytes = std::fs::read(source).ok()?;
    let data: Value = serde_json::from_slice(&bytes).ok()?;
    let groups: Vec<_> = data
        .get("Groups")?
        .as_array()?
        .iter()
        .filter_map(|group| {
            let key = group.get("UniqueId")?.as_str()?;
            let title = group.get("Title")?.as_str()?;
            let items = group.get("Items")?.as_array()?.iter().filter_map(|item| {
                Some(SemanticZoomSampleItem {
                    key: format!("{key}/{}", item.get("UniqueId")?.as_str()?).into(),
                    title: item.get("Title")?.as_str()?.into(),
                    subtitle: item
                        .get("Subtitle")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .into(),
                })
            });
            Some(SemanticZoomSampleGroup {
                key: key.into(),
                title: title.into(),
                items: ModelRc::new(VecModel::from_iter(items)),
            })
        })
        .collect();
    (!groups.is_empty()).then_some(groups)
}

fn fallback_groups() -> Vec<SemanticZoomSampleGroup> {
    [
        (
            "fundamentals",
            "Fundamentals",
            [
                ("Resources", "Share values and styles across the interface."),
                ("Templates", "Compose the appearance of a control."),
            ],
        ),
        (
            "collections",
            "Collections",
            [
                ("Grid", "Browse a collection of visual items."),
                ("Tree", "Follow a hierarchy of related items."),
            ],
        ),
        (
            "navigation",
            "Navigation",
            [
                ("Breadcrumbs", "Return to an earlier location."),
                ("Tabs", "Switch among open views."),
            ],
        ),
        (
            "text",
            "Text",
            [
                ("Text field", "Enter or edit a value."),
                ("Search", "Find an item by name."),
            ],
        ),
    ]
    .into_iter()
    .map(|(key, title, items)| SemanticZoomSampleGroup {
        key: key.into(),
        title: title.into(),
        items: ModelRc::new(VecModel::from_iter(items.into_iter().enumerate().map(
            |(index, (title, subtitle))| SemanticZoomSampleItem {
                key: format!("{key}/{index}").into(),
                title: title.into(),
                subtitle: subtitle.into(),
            },
        ))),
    })
    .collect()
}

fn arrange(
    groups: ModelRc<SemanticZoomSampleGroup>,
    width: f32,
    rtl: bool,
) -> SemanticZoomSampleLayout {
    const CELL_WIDTH: f32 = 324.0;
    const CELL_HEIGHT: f32 = 68.0;
    const GAP: f32 = 4.0;
    const HEADER_HEIGHT: f32 = 48.0;
    let columns = ((width + GAP) / (CELL_WIDTH + GAP)).floor().max(1.0) as usize;
    let tile_width = width.min(CELL_WIDTH);
    let mut cells = Vec::new();
    let mut top = 0.0;
    for group in groups.iter() {
        cells.push(SemanticZoomSampleCell {
            key: group.key.clone(),
            group_key: group.key.clone(),
            title: group.title,
            y: top,
            width,
            height: HEADER_HEIGHT,
            header: true,
            ..Default::default()
        });
        top += HEADER_HEIGHT;
        for (index, item) in group.items.iter().enumerate() {
            let left = (index % columns) as f32 * (CELL_WIDTH + GAP);
            cells.push(SemanticZoomSampleCell {
                key: item.key,
                group_key: group.key.clone(),
                title: item.title,
                subtitle: item.subtitle,
                x: if rtl { width - tile_width - left } else { left },
                y: top + (index / columns) as f32 * (CELL_HEIGHT + GAP),
                width: tile_width,
                height: CELL_HEIGHT,
                header: false,
            });
        }
        top += group.items.row_count().div_ceil(columns) as f32 * (CELL_HEIGHT + GAP);
    }
    SemanticZoomSampleLayout {
        cells: ModelRc::new(VecModel::from(cells)),
        height: top,
    }
}

fn move_detail(cells: ModelRc<SemanticZoomSampleCell>, key: SharedString, direction: i32) -> i32 {
    let Some(current) = cells.iter().find(|item| item.key == key) else {
        return -1;
    };
    let center_x = current.x + current.width / 2.0;
    cells
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let dx = item.x + item.width / 2.0 - center_x;
            let dy = item.y - current.y;
            let priority = match direction {
                0 if dy == 0.0 && dx < 0.0 => (-dx, 0.0),
                1 if dy == 0.0 && dx > 0.0 => (dx, 0.0),
                2 if dy < 0.0 => (-dy, dx.abs()),
                3 if dy > 0.0 => (dy, dx.abs()),
                _ => return None,
            };
            Some((index, priority))
        })
        .min_by(|(_, (main_a, cross_a)), (_, (main_b, cross_b))| {
            main_a
                .total_cmp(main_b)
                .then_with(|| cross_a.total_cmp(cross_b))
        })
        .map_or(-1, |(index, _)| index as i32)
}

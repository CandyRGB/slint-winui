use crate::{
    AppBarLabelPosition, CommandBarBehavior, CommandBarEntry, CommandBarItem, CommandBarItemKind,
    CommandBarLayout, CommandBarOverflowButtonVisibility,
};
use slint::{Model, ModelExt, ModelRc, VecModel};

const MORE_WIDTH: f32 = 48.0;
const END_GAP: f32 = 6.0;
const SEPARATOR_WIDTH: f32 = 6.0;
const OVERFLOW_SEPARATOR_HEIGHT: f32 = 8.0;
const OVERFLOW_BUTTON_HEIGHT: f32 = 36.0;

pub fn install(behavior: CommandBarBehavior<'_>) {
    behavior.on_arrange(
        |primary,
         secondary,
         width,
         content_width,
         item_width,
         dynamic,
         label_position,
         visibility,
         reveal| {
            arrange(
                entries(&primary, false, item_width),
                entries(&secondary, true, item_width),
                LayoutOptions {
                    width,
                    content_width,
                    dynamic,
                    label_position,
                    visibility,
                    reveal,
                },
            )
        },
    );
    behavior.on_next_item(next_item);
    behavior.on_index_of(index_of);
    behavior.on_accessible_index(accessible_index);
    behavior.on_has_actions(|items| {
        items.model_tracker().track_row_count_changes();
        (0..items.row_count()).any(|index| {
            items
                .row_data_tracked(index)
                .is_some_and(|item| !item.hidden && item.kind != CommandBarItemKind::Separator)
        })
    });
    behavior.on_toggle(toggle);
}

fn entries(
    items: &ModelRc<CommandBarItem>,
    secondary: bool,
    default_width: f32,
) -> Vec<CommandBarEntry> {
    items.model_tracker().track_row_count_changes();
    (0..items.row_count())
        .filter_map(|source_index| {
            let item = items.row_data_tracked(source_index)?;
            if item.hidden {
                return None;
            }
            let width = if item.kind == CommandBarItemKind::Separator {
                SEPARATOR_WIDTH
            } else if item.width > 0.0 {
                (item.width / 2.0).round() * 2.0
            } else {
                default_width
            };
            Some(CommandBarEntry {
                item,
                source_index: source_index as i32,
                secondary,
                width,
            })
        })
        .collect()
}

struct LayoutOptions {
    width: f32,
    content_width: f32,
    dynamic: bool,
    label_position: AppBarLabelPosition,
    visibility: CommandBarOverflowButtonVisibility,
    reveal: bool,
}

fn is_separator(entry: &CommandBarEntry) -> bool {
    entry.item.kind == CommandBarItemKind::Separator
}

fn arrange(
    mut primary: Vec<CommandBarEntry>,
    mut secondary: Vec<CommandBarEntry>,
    options: LayoutOptions,
) -> CommandBarLayout {
    trim_separators(&mut primary);
    trim_separators(&mut secondary);

    let mut primary_width: f32 = primary.iter().map(|entry| entry.width).sum();
    let mut more = match options.visibility {
        CommandBarOverflowButtonVisibility::Auto => {
            options.reveal
                || !secondary.is_empty()
                || has_bottom_labels(&primary, options.label_position)
                || (options.dynamic
                    && primary_width + options.content_width + END_GAP > options.width
                    && !primary.is_empty())
        }
        CommandBarOverflowButtonVisibility::Visible => true,
        CommandBarOverflowButtonVisibility::Collapsed => false,
    };

    let mut overflow = Vec::new();
    if options.dynamic {
        let available =
            options.width - options.content_width - if more { MORE_WIDTH } else { END_GAP };
        let mut keep = vec![true; primary.len()];
        let mut removal_order: Vec<_> = (0..primary.len()).collect();
        // Positive orders move in ascending groups. Default items move from the trailing end.
        removal_order.sort_by_key(|&index| {
            let order = primary[index].item.overflow_order;
            (order <= 0, order.max(0), std::cmp::Reverse(index))
        });
        let mut candidates = removal_order.into_iter().peekable();
        while primary_width > available {
            let Some(index) = candidates.next() else {
                break;
            };
            if !keep[index] {
                continue;
            }
            let order = primary[index].item.overflow_order;
            move_to_overflow(&primary, &mut keep, &mut primary_width, index);
            while order > 0
                && candidates
                    .peek()
                    .is_some_and(|&next| primary[next].item.overflow_order == order)
            {
                let next = candidates.next().expect("peeked overflow candidate");
                move_to_overflow(&primary, &mut keep, &mut primary_width, next);
            }
            trim_moved_separators(&primary, &mut keep, &mut primary_width);
        }
        let visible = primary.into_iter().zip(keep);
        primary = Vec::new();
        for (entry, stays) in visible {
            if stays {
                primary.push(entry);
            } else {
                overflow.push(entry);
            }
        }
        trim_separators(&mut overflow);
    }

    if !overflow.is_empty() && !secondary.is_empty() {
        overflow.push(CommandBarEntry {
            item: CommandBarItem {
                kind: CommandBarItemKind::Separator,
                ..Default::default()
            },
            ..Default::default()
        });
    }
    overflow.append(&mut secondary);
    if options.visibility == CommandBarOverflowButtonVisibility::Auto {
        more |= !overflow.is_empty();
    }

    CommandBarLayout {
        primary_width,
        overflow_height: 8.0
            + overflow
                .iter()
                .map(|entry| {
                    if is_separator(entry) {
                        OVERFLOW_SEPARATOR_HEIGHT
                    } else {
                        OVERFLOW_BUTTON_HEIGHT
                    }
                })
                .sum::<f32>(),
        more,
        has_bottom_labels: has_bottom_labels(&primary, options.label_position),
        overflow_has_icons: overflow
            .iter()
            .any(|entry| entry.item.icon.size().width > 0),
        overflow_has_toggles: overflow
            .iter()
            .any(|entry| entry.item.kind == CommandBarItemKind::Toggle),
        primary: ModelRc::new(VecModel::from(primary)),
        overflow: ModelRc::new(VecModel::from(overflow)),
    }
}

fn has_bottom_labels(entries: &[CommandBarEntry], position: AppBarLabelPosition) -> bool {
    position == AppBarLabelPosition::Bottom
        && entries
            .iter()
            .any(|entry| !is_separator(entry) && !entry.item.label.is_empty())
}

fn trim_separators(entries: &mut Vec<CommandBarEntry>) {
    let mut previous_separator = true;
    entries.retain(|entry| {
        let separator = is_separator(entry);
        let retain = !separator || !previous_separator;
        previous_separator = separator;
        retain
    });
    if entries.last().is_some_and(is_separator) {
        entries.pop();
    }
}

fn move_to_overflow(entries: &[CommandBarEntry], keep: &mut [bool], width: &mut f32, index: usize) {
    if keep[index] {
        keep[index] = false;
        *width -= entries[index].width;
    }
}

fn trim_moved_separators(entries: &[CommandBarEntry], keep: &mut [bool], width: &mut f32) {
    let mut previous_separator = true;
    let mut last = None;
    for (index, entry) in entries.iter().enumerate() {
        if keep[index] {
            let separator = is_separator(entry);
            if separator && previous_separator {
                move_to_overflow(entries, keep, width, index);
            } else {
                last = Some(index);
            }
            previous_separator = separator;
        }
    }
    if let Some(index) = last.filter(|&index| is_separator(&entries[index])) {
        move_to_overflow(entries, keep, width, index);
    }
}

fn next_item(entries: ModelRc<CommandBarEntry>, index: i32, forward: bool) -> i32 {
    let count = entries.row_count() as i32;
    let is_focusable = |index: &i32| {
        entries
            .row_data_tracked(*index as usize)
            .is_some_and(|entry| !entry.item.disabled && !is_separator(&entry))
    };
    if forward {
        (index.saturating_add(1).max(0)..count).find(is_focusable)
    } else {
        (0..if index < 0 { count } else { index.min(count) })
            .rev()
            .find(is_focusable)
    }
    .unwrap_or(-1)
}

fn accessible_index(entries: ModelRc<CommandBarEntry>, index: i32) -> i32 {
    (0..(index.max(0) as usize).min(entries.row_count()))
        .filter_map(|index| entries.row_data_tracked(index))
        .filter(|entry| !is_separator(entry))
        .count() as i32
}

fn index_of(entries: ModelRc<CommandBarEntry>, id: slint::SharedString) -> i32 {
    if id.is_empty() {
        return -1;
    }
    entries
        .iter()
        .position(|entry| entry.item.id == id && !entry.item.disabled && !is_separator(&entry))
        .map_or(-1, |index| index as i32)
}

fn toggle(items: ModelRc<CommandBarItem>, index: i32) -> CommandBarItem {
    let Some((index, mut item)) = usize::try_from(index)
        .ok()
        .and_then(|index| items.row_data(index).map(|item| (index, item)))
    else {
        return CommandBarItem::default();
    };
    if !item.disabled && item.kind == CommandBarItemKind::Toggle {
        item.checked = !item.checked;
        items.set_row_data(index, item.clone());
    }
    item
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(id: &str, order: i32) -> CommandBarItem {
        CommandBarItem {
            id: id.into(),
            label: id.into(),
            overflow_order: order,
            ..Default::default()
        }
    }

    fn layout(
        primary: Vec<CommandBarItem>,
        secondary: Vec<CommandBarItem>,
        width: f32,
    ) -> CommandBarLayout {
        arrange(
            entries(&ModelRc::new(VecModel::from(primary)), false, 68.0),
            entries(&ModelRc::new(VecModel::from(secondary)), true, 68.0),
            LayoutOptions {
                width,
                content_width: 0.0,
                dynamic: true,
                label_position: AppBarLabelPosition::Bottom,
                visibility: CommandBarOverflowButtonVisibility::Auto,
                reveal: false,
            },
        )
    }

    fn ids(model: &ModelRc<CommandBarEntry>) -> Vec<String> {
        model
            .iter()
            .map(|entry| entry.item.id.to_string())
            .collect()
    }

    #[test]
    fn moves_trailing_commands_in_original_order_and_separates_secondary_commands() {
        let layout = layout(
            vec![action("one", 0), action("two", 0), action("three", 0)],
            vec![action("four", 0)],
            184.0,
        );
        assert_eq!(ids(&layout.primary), ["one", "two"]);
        assert_eq!(ids(&layout.overflow), ["three", "", "four"]);
        assert_eq!(layout.overflow.row_data(2).unwrap().source_index, 0);
        assert!(layout.overflow.row_data(2).unwrap().secondary);
    }

    #[test]
    fn moves_a_positive_order_group_together_before_default_items() {
        let layout = layout(
            vec![
                action("one", 1),
                action("two", 0),
                action("three", 1),
                action("four", 2),
            ],
            vec![],
            252.0,
        );
        assert_eq!(ids(&layout.primary), ["two", "four"]);
        assert_eq!(ids(&layout.overflow), ["one", "three"]);
    }

    #[test]
    fn focus_skips_disabled_commands_and_does_not_wrap() {
        let layout = layout(
            vec![
                action("one", 0),
                CommandBarItem {
                    kind: CommandBarItemKind::Separator,
                    ..Default::default()
                },
                CommandBarItem {
                    disabled: true,
                    ..action("two", 0)
                },
                action("three", 0),
            ],
            vec![],
            480.0,
        );
        let primary = layout.primary;
        assert_eq!(next_item(primary.clone(), 0, true), 3);
        assert_eq!(next_item(primary.clone(), 3, false), 0);
        assert_eq!(next_item(primary, 3, true), -1);
    }
}

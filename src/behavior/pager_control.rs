use slint::{Model, ModelRc, VecModel};

use crate::{ComboBoxItem, PagerControlBehavior};

pub fn install(behavior: PagerControlBehavior<'_>) {
    behavior.on_panel_items(|count, selected, show_edges| {
        ModelRc::new(VecModel::from(panel_items(count, selected, show_edges)))
    });
    behavior.on_combo_items(|count| {
        ModelRc::new(PageOptions {
            count: usize::try_from(count).unwrap_or(100),
            tracker: Default::default(),
        })
    });
    behavior.on_number_to_index(|number| (number as i32).saturating_sub(1));
}

struct PageOptions {
    count: usize,
    tracker: slint::ModelNotify,
}

impl Model for PageOptions {
    type Data = ComboBoxItem;

    fn row_count(&self) -> usize {
        self.count
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        (row < self.count).then(|| ComboBoxItem {
            text: (row + 1).to_string().into(),
            ..Default::default()
        })
    }

    fn model_tracker(&self) -> &dyn slint::ModelTracker {
        &self.tracker
    }
}

// Rows are one-based so zero can represent either ellipsis.
fn panel_items(count: i32, selected: i32, show_edges: bool) -> Vec<i32> {
    let selected = selected.clamp(0, i32::MAX - 1);
    if count < 0 {
        return if selected < 3 {
            (1..=5).collect()
        } else {
            vec![1, 0, selected, selected + 1, selected.saturating_add(2)]
        };
    }
    if count < 8 {
        return (1..=count).collect();
    }
    if selected < 4 {
        let mut pages: Vec<_> = (1..=5).collect();
        if show_edges {
            pages.extend([0, count]);
        }
        return pages;
    }
    if selected >= count - 4 {
        let mut pages = if show_edges { vec![1, 0] } else { Vec::new() };
        pages.extend(count - 4..=count);
        return pages;
    }
    let neighbors = [selected, selected + 1, selected + 2];
    if show_edges {
        [vec![1, 0], neighbors.to_vec(), vec![0, count]].concat()
    } else {
        neighbors.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_panel_rebuilds_for_each_position_and_edge_policy() {
        assert_eq!(panel_items(7, 3, true), [1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(panel_items(20, 0, true), [1, 2, 3, 4, 5, 0, 20]);
        assert_eq!(panel_items(20, 4, true), [1, 0, 4, 5, 6, 0, 20]);
        assert_eq!(panel_items(20, 4, false), [4, 5, 6]);
        assert_eq!(panel_items(20, 19, true), [1, 0, 16, 17, 18, 19, 20]);
        assert_eq!(panel_items(-1, 2, false), [1, 2, 3, 4, 5]);
        assert_eq!(panel_items(-1, 99, false), [1, 0, 99, 100, 101]);
        assert!(panel_items(0, 0, true).is_empty());
    }
}

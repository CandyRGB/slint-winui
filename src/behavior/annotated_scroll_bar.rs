use slint::{Model, ModelRc, VecModel};

use crate::{AnnotatedScrollBarBehavior, AnnotatedScrollBarLabel, AnnotatedScrollBarMarker};

pub fn install(behavior: AnnotatedScrollBarBehavior<'_>) {
    behavior.on_layout(layout);
}

fn layout(
    labels: ModelRc<AnnotatedScrollBarLabel>,
    minimum: f32,
    scale: f32,
    height: f32,
    label_height: f32,
) -> ModelRc<AnnotatedScrollBarMarker> {
    let mut markers = labels
        .iter()
        .filter_map(|label| {
            let position = (label.offset - minimum) * scale;
            position.is_finite().then_some(AnnotatedScrollBarMarker {
                text: label.text,
                position,
                shown: false,
            })
        })
        .collect::<Vec<_>>();
    let count = markers.len();
    let mut lower: Option<usize> = None;
    for index in (0..count).rev() {
        let top = markers[index].position;
        let bottom = top + label_height;
        if top < 0.0 || bottom > height {
            continue;
        }
        if let Some(previous) = lower.filter(|&previous| markers[previous].position <= bottom) {
            // Native collision resolution favors the lower label, except that the
            // first label can displace an intermediate one.
            if index == 0 && previous + 1 < count {
                markers[previous].shown = false;
                markers[index].shown = true;
            }
            continue;
        }
        markers[index].shown = true;
        lower = Some(index);
    }
    for marker in &mut markers {
        marker.position = (marker.position / 2.0).round() * 2.0;
    }
    ModelRc::new(VecModel::from(markers))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visible_offsets(offsets: &[f32], minimum: f32, height: f32) -> Vec<f32> {
        let labels = offsets.iter().map(|&offset| AnnotatedScrollBarLabel {
            offset,
            ..Default::default()
        });
        layout(
            ModelRc::new(VecModel::from_iter(labels)),
            minimum,
            1.0,
            height,
            14.0,
        )
        .iter()
        .filter(|marker| marker.shown)
        .map(|marker| marker.position)
        .collect()
    }

    #[test]
    fn labels_at_the_edges_take_precedence_when_space_is_tight() {
        assert_eq!(visible_offsets(&[0.0, 6.0, 30.0], 0.0, 60.0), [0.0, 30.0]);
        assert_eq!(visible_offsets(&[0.0, 6.0], 0.0, 60.0), [6.0]);
        assert_eq!(
            visible_offsets(&[-30.0, 10.0, 28.0, 62.0], 10.0, 50.0),
            [0.0, 18.0]
        );
    }
}

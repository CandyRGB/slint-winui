use slint::{Model, SharedString};

use crate::{
    ScrollAnchorBehavior, ScrollAnchorCandidate, ScrollAnchorLayout, ScrollAnchorRequest,
    ScrollAnchorResult,
};

const EDGE_TOLERANCE: f32 = 0.1;

pub fn install(behavior: ScrollAnchorBehavior<'_>) {
    behavior.on_resolve(resolve);
    behavior.set_ready(true);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Unanchored,
    Element,
    FarEdge,
}

#[derive(Clone, Copy)]
struct Axis {
    offset: f32,
    ratio: f32,
    old_viewport: f32,
    new_viewport: f32,
    old_extent: f32,
    new_extent: f32,
    zoom: f32,
    centered: bool,
    enabled: bool,
}

impl Axis {
    fn mode(self) -> Mode {
        if !self.enabled || !(0.0..=1.0).contains(&self.ratio) {
            Mode::Unanchored
        } else if self.ratio == 1.0
            && self.offset + self.old_viewport > self.old_extent - EDGE_TOLERANCE
        {
            Mode::FarEdge
        } else if self.ratio == 0.0 && self.offset < EDGE_TOLERANCE {
            Mode::Unanchored
        } else {
            Mode::Element
        }
    }

    fn origin(self, viewport: f32, extent: f32) -> f32 {
        if self.centered {
            ((viewport - extent) * 0.5).max(0.0)
        } else {
            0.0
        }
    }

    fn intersects(self, start: f32, size: f32) -> bool {
        let start = start * self.zoom + self.origin(self.old_viewport, self.old_extent);
        start < self.offset + self.old_viewport && start + size * self.zoom > self.offset
    }

    fn distance(self, start: f32, size: f32) -> f64 {
        if self.mode() != Mode::Element {
            return 0.0;
        }
        let point = self.offset + self.ratio * self.old_viewport;
        let start = start * self.zoom + self.origin(self.old_viewport, self.old_extent);
        f64::from(point - start).powi(2) + f64::from(point - start - size * self.zoom).powi(2)
    }

    fn corrected(self, bounds: Option<((f32, f32), (f32, f32))>) -> f32 {
        let delta = match self.mode() {
            Mode::Unanchored => 0.0,
            Mode::FarEdge => {
                (self.new_extent - self.old_extent).max(0.0)
                    + (self.old_viewport - self.new_viewport).max(0.0)
            }
            Mode::Element => {
                bounds.map_or(0.0, |((old_start, old_size), (new_start, new_size))| {
                    let old_point = old_start + self.ratio * old_size;
                    let new_point = new_start + self.ratio * new_size;
                    (new_point - old_point) * self.zoom
                        + self.origin(self.new_viewport, self.new_extent)
                        - self.origin(self.old_viewport, self.old_extent)
                        - (self.new_viewport - self.old_viewport) * self.ratio
                })
            }
        };
        let limit = if self.enabled {
            (self.new_extent - self.new_viewport).max(0.0)
        } else {
            0.0
        };
        (self.offset + delta).clamp(0.0, limit)
    }
}

fn valid(candidate: &ScrollAnchorCandidate) -> bool {
    !candidate.id.is_empty()
        && candidate.x.is_finite()
        && candidate.y.is_finite()
        && candidate.width.is_finite()
        && candidate.height.is_finite()
        && candidate.width > 0.0
        && candidate.height > 0.0
}

fn resolve(
    previous: ScrollAnchorLayout,
    current: ScrollAnchorLayout,
    request: ScrollAnchorRequest,
) -> ScrollAnchorResult {
    let horizontal = Axis {
        offset: request.x,
        ratio: request.horizontal_ratio,
        old_viewport: previous.viewport_width,
        new_viewport: current.viewport_width,
        old_extent: previous.extent_width.max(0.0) * request.zoom,
        new_extent: current.extent_width.max(0.0) * request.zoom,
        zoom: request.zoom,
        centered: request.center_content,
        enabled: request.horizontal_enabled,
    };
    let vertical = Axis {
        offset: request.y,
        ratio: request.vertical_ratio,
        old_viewport: previous.viewport_height,
        new_viewport: current.viewport_height,
        old_extent: previous.extent_height.max(0.0) * request.zoom,
        new_extent: current.extent_height.max(0.0) * request.zoom,
        zoom: request.zoom,
        centered: request.center_content,
        enabled: request.vertical_enabled,
    };

    let mut best = None;
    let mut best_distance = f64::INFINITY;
    if horizontal.mode() == Mode::Element || vertical.mode() == Mode::Element {
        for candidate in previous.candidates.iter().filter(valid) {
            if candidate.id == request.preferred_id {
                best = Some(candidate);
                break;
            }
            if !horizontal.intersects(candidate.x, candidate.width)
                || !vertical.intersects(candidate.y, candidate.height)
            {
                continue;
            }
            let distance = horizontal.distance(candidate.x, candidate.width)
                + vertical.distance(candidate.y, candidate.height);
            if distance <= best_distance {
                best_distance = distance;
                best = Some(candidate);
            }
        }
    }

    let pair = best.and_then(|old| {
        let new = current
            .candidates
            .iter()
            .find(|candidate| candidate.id == old.id && valid(candidate))?;
        Some((old, new))
    });
    let id = pair
        .as_ref()
        .map_or_else(SharedString::default, |(old, _)| old.id.clone());
    ScrollAnchorResult {
        id,
        x: horizontal.corrected(
            pair.as_ref()
                .map(|(old, new)| ((old.x, old.width), (new.x, new.width))),
        ),
        y: vertical.corrected(
            pair.as_ref()
                .map(|(old, new)| ((old.y, old.height), (new.y, new.height))),
        ),
    }
}

#[cfg(test)]
mod tests {
    use slint::{ModelRc, VecModel};

    use super::*;

    fn layout(viewport: f32, extent: f32, rows: &[(&str, f32)]) -> ScrollAnchorLayout {
        ScrollAnchorLayout {
            viewport_width: 400.0,
            viewport_height: viewport,
            extent_width: 400.0,
            extent_height: extent,
            candidates: ModelRc::new(VecModel::from(
                rows.iter()
                    .map(|&(id, y)| ScrollAnchorCandidate {
                        id: id.into(),
                        y,
                        width: 400.0,
                        height: 96.0,
                        ..Default::default()
                    })
                    .collect::<Vec<_>>(),
            )),
        }
    }

    fn request(y: f32, ratio: f32) -> ScrollAnchorRequest {
        ScrollAnchorRequest {
            y,
            zoom: 1.0,
            vertical_ratio: ratio,
            vertical_enabled: true,
            ..Default::default()
        }
    }

    #[test]
    fn keeps_the_closest_visible_row_when_content_is_inserted_above() {
        let before = layout(
            300.0,
            576.0,
            &[("invisible", 100.0), ("a", 192.0), ("b", 288.0)],
        );
        let after = layout(300.0, 640.0, &[("a", 256.0), ("b", 352.0)]);
        let result = resolve(before.clone(), after.clone(), request(240.0, 0.0));
        assert_eq!((result.id.as_str(), result.y), ("a", 304.0));
        let result = resolve(before, after, request(0.0, 0.0));
        assert_eq!((result.id.as_str(), result.y), ("", 0.0));
    }

    #[test]
    fn centered_anchor_compensates_for_viewport_resize_and_zoom() {
        let before = layout(300.0, 700.0, &[("a", 192.0)]);
        let after = layout(240.0, 700.0, &[("a", 224.0)]);
        let result = resolve(
            before,
            after,
            ScrollAnchorRequest {
                zoom: 2.0,
                ..request(288.0, 0.5)
            },
        );
        assert_eq!((result.id.as_str(), result.y), ("a", 382.0));
    }

    #[test]
    fn chooses_one_visible_candidate_for_both_axes() {
        let make_layout = |left, top| ScrollAnchorLayout {
            viewport_width: 200.0,
            viewport_height: 200.0,
            extent_width: 600.0,
            extent_height: 600.0,
            candidates: ModelRc::new(VecModel::from(vec![
                ScrollAnchorCandidate {
                    id: "visible".into(),
                    x: left,
                    y: top,
                    width: 80.0,
                    height: 80.0,
                },
                ScrollAnchorCandidate {
                    id: "outside".into(),
                    x: 100.0,
                    y: 301.0,
                    width: 80.0,
                    height: 80.0,
                },
            ])),
        };
        let result = resolve(
            make_layout(140.0, 140.0),
            make_layout(172.0, 204.0),
            ScrollAnchorRequest {
                x: 100.0,
                horizontal_enabled: true,
                horizontal_ratio: 0.5,
                ..request(100.0, 0.5)
            },
        );
        assert_eq!(
            (result.id.as_str(), result.x, result.y),
            ("visible", 132.0, 164.0)
        );
    }

    #[test]
    fn far_edge_follows_growth_and_the_reduced_viewport() {
        let before = layout(300.0, 576.0, &[]);
        let after = layout(240.0, 640.0, &[]);
        let result = resolve(before.clone(), after.clone(), request(276.0, 1.0));
        assert_eq!((result.id.as_str(), result.y), ("", 400.0));
        assert_eq!(resolve(before.clone(), after, request(200.0, 1.0)).y, 200.0);
        let smaller = layout(300.0, 512.0, &[]);
        assert_eq!(resolve(before, smaller, request(276.0, 1.0)).y, 212.0);
    }

    #[test]
    fn preferred_candidate_can_override_selection_and_removed_candidates_do_not_move_view() {
        let before = layout(300.0, 576.0, &[("near", 192.0), ("preferred", 288.0)]);
        let request = ScrollAnchorRequest {
            preferred_id: "preferred".into(),
            ..request(240.0, 0.0)
        };
        let after = layout(300.0, 660.0, &[("near", 192.0), ("preferred", 368.0)]);
        let result = resolve(before.clone(), after, request.clone());
        assert_eq!((result.id.as_str(), result.y), ("preferred", 320.0));
        let after = layout(300.0, 576.0, &[("near", 256.0)]);
        let result = resolve(before, after, request);
        assert_eq!((result.id.as_str(), result.y), ("", 240.0));
    }
}

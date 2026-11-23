use slint::Model;

use crate::{
    ScrollSnapAlignment, ScrollSnapBehavior, ScrollSnapIntent, ScrollSnapPoints, ScrollSnapRange,
    ScrollSnapRequest,
};

const EPSILON: f64 = 0.001;

pub fn install(behavior: ScrollSnapBehavior<'_>) {
    behavior.on_resolve(resolve);
    behavior.set_ready(true);
}

#[derive(Clone, Copy)]
struct Space {
    viewport: f64,
    zoom: f64,
    limit: f64,
}

impl Space {
    fn position(self, value: f32, alignment: ScrollSnapAlignment) -> f64 {
        let anchor = match alignment {
            ScrollSnapAlignment::Near => 0.0,
            ScrollSnapAlignment::Center => self.viewport * 0.5,
            ScrollSnapAlignment::Far => self.viewport,
        };
        f64::from(value) * self.zoom - anchor
    }
}

#[derive(Clone, Copy)]
struct Range {
    offset: f64,
    interval: f64,
    start: f64,
    end: f64,
}

impl Range {
    fn new(input: ScrollSnapRange, space: Space) -> Option<Self> {
        let range = Self {
            offset: space.position(input.offset, input.alignment),
            interval: f64::from(input.interval) * space.zoom,
            start: space.position(input.start, input.alignment),
            end: space.position(input.end, input.alignment),
        };
        (range.offset.is_finite()
            && range.interval.is_finite()
            && range.start.is_finite()
            && range.end.is_finite()
            && range.interval > EPSILON
            && range.start < range.end
            && range.offset >= range.start
            && range.offset <= range.end)
            .then_some(range)
    }

    fn nearest(self, target: f64, direction: Direction, origin: f64) -> Option<f64> {
        let mut low = self.start;
        let mut high = self.end;
        match direction {
            Direction::Forward => low = low.max(origin + EPSILON),
            Direction::Backward => high = high.min(origin - EPSILON),
            Direction::Any => {}
        }
        let first = ((low - self.offset) / self.interval).ceil();
        let last = ((high - self.offset) / self.interval).floor();
        if first > last {
            return None;
        }

        let raw_index = (target - self.offset) / self.interval;
        let lower = raw_index.floor();
        let nearest = lower + if raw_index - lower > 0.5 { 1.0 } else { 0.0 };
        Some(self.offset + nearest.clamp(first, last) * self.interval)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Any,
    Forward,
    Backward,
}

impl Direction {
    fn accepts(self, point: f64, origin: f64) -> bool {
        match self {
            Self::Any => true,
            Self::Forward => point > origin + EPSILON,
            Self::Backward => point < origin - EPSILON,
        }
    }
}

struct SnapSet {
    singles: Vec<f64>,
    ranges: Vec<Range>,
}

impl SnapSet {
    fn new(config: &ScrollSnapPoints, space: Space) -> Self {
        let mut ranges: Vec<_> = config
            .ranges
            .iter()
            .filter_map(|range| Range::new(range, space))
            .collect();
        ranges.sort_by(|a, b| a.start.total_cmp(&b.start));
        // Native collections reject overlapping ranges. Ignore later overlaps so bindings can
        // update start and end independently without interrupting the active scroll operation.
        let mut previous_end = f64::NEG_INFINITY;
        ranges.retain(|range| {
            if range.start < previous_end {
                false
            } else {
                previous_end = range.end;
                true
            }
        });
        let mut singles: Vec<_> = config
            .points
            .iter()
            .map(|point| space.position(point.value, point.alignment))
            .chain(
                config
                    .aligned_points
                    .iter()
                    .map(|value| space.position(value, config.alignment)),
            )
            .filter(|point| point.is_finite())
            .collect();
        singles.sort_by(f64::total_cmp);
        singles.dedup_by(|a, b| (*a - *b).abs() < EPSILON);
        singles.retain(|&point| {
            let preceding = ranges.partition_point(|range| range.start < point);
            preceding == 0 || point >= ranges[preceding - 1].end
        });
        Self { singles, ranges }
    }

    fn nearest(&self, target: f64, origin: f64, direction: Direction, limit: f64) -> Option<f64> {
        let mut best: Option<f64> = None;
        let mut consider = |candidate: f64| {
            if direction.accepts(candidate.clamp(0.0, limit), origin)
                && best.is_none_or(|current| {
                    let distance = (target - candidate).abs();
                    let previous = (target - current).abs();
                    distance < previous || (distance == previous && candidate < current)
                })
            {
                best = Some(candidate);
            }
        };

        if direction == Direction::Any {
            let right = self.ranges.partition_point(|range| range.start < target);
            let preceding = right
                .checked_sub(1)
                .and_then(|index| self.ranges.get(index));
            let following = self.ranges.get(right);
            if self
                .singles
                .binary_search_by(|point| point.total_cmp(&target))
                .is_ok()
            {
                return Some(target);
            }
            if let Some(range) = preceding
                .filter(|range| target <= range.end)
                .or_else(|| following.filter(|range| target == range.start))
            {
                return range
                    .nearest(target, direction, origin)
                    .map(|point| point.clamp(0.0, limit));
            }
            let start = preceding.map_or(f64::NEG_INFINITY, |range| range.end);
            let end = following.map_or(f64::INFINITY, |range| range.start);
            let first = self.singles.partition_point(|&point| point < start);
            for &point in &self.singles[first..] {
                if point > end {
                    break;
                }
                consider(point);
            }
        } else {
            for &point in &self.singles {
                consider(point);
            }
            for &range in &self.ranges {
                if let Some(point) = range.nearest(target, direction, origin) {
                    consider(point);
                }
            }
        }
        best.map(|candidate| candidate.clamp(0.0, limit))
    }
}

fn resolve(config: ScrollSnapPoints, request: ScrollSnapRequest) -> f32 {
    let space = Space {
        viewport: f64::from(request.viewport.max(0.0)),
        zoom: f64::from(request.zoom.max(0.0001)),
        limit: f64::from(request.limit.max(0.0)),
    };
    let target = f64::from(request.target).clamp(0.0, space.limit);
    let origin = f64::from(request.origin).clamp(0.0, space.limit);
    let set = SnapSet::new(&config, space);
    let travel = target - origin;
    let use_direction = travel.abs() >= EPSILON
        && match request.intent {
            ScrollSnapIntent::Nearest => false,
            ScrollSnapIntent::Directional => true,
            ScrollSnapIntent::Impulse => set
                .nearest(origin, origin, Direction::Any, space.limit)
                .is_some_and(|point| (origin - point).abs() < EPSILON),
        };
    let direction = if !use_direction {
        Direction::Any
    } else if travel > 0.0 {
        Direction::Forward
    } else {
        Direction::Backward
    };
    set.nearest(target, origin, direction, space.limit)
        .unwrap_or(target) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScrollSnapPoint;
    use std::rc::Rc;

    fn request(target: f32, origin: f32, intent: ScrollSnapIntent) -> ScrollSnapRequest {
        ScrollSnapRequest {
            target,
            origin,
            intent,
            limit: 1000.0,
            viewport: 400.0,
            zoom: 1.0,
        }
    }

    fn range(offset: f32, interval: f32, start: f32, end: f32) -> ScrollSnapRange {
        ScrollSnapRange {
            offset,
            interval,
            start,
            end,
            alignment: ScrollSnapAlignment::Near,
        }
    }

    #[test]
    fn discrete_points_use_viewport_alignment_and_lower_midpoint_ties() {
        let config = ScrollSnapPoints {
            aligned_points: Rc::new(slint::VecModel::from(vec![96.0, 0.0, 96.0])).into(),
            points: Rc::new(slint::VecModel::from(vec![
                ScrollSnapPoint {
                    value: 840.0,
                    alignment: ScrollSnapAlignment::Far,
                },
                ScrollSnapPoint {
                    value: 480.0,
                    alignment: ScrollSnapAlignment::Center,
                },
            ]))
            .into(),
            ..Default::default()
        };
        for (target, expected) in [(48.0, 0.0), (49.0, 96.0), (280.0, 280.0), (420.0, 440.0)] {
            assert_eq!(
                resolve(
                    config.clone(),
                    request(target, 0.0, ScrollSnapIntent::Nearest)
                ),
                expected
            );
        }
        let mut zoomed = request(750.0, 0.0, ScrollSnapIntent::Nearest);
        zoomed.zoom = 2.0;
        assert_eq!(resolve(config, zoomed), 760.0);
    }

    #[test]
    fn repeated_points_respect_bounds_and_outer_gaps() {
        let config = ScrollSnapPoints {
            aligned_points: Rc::new(slint::VecModel::from(vec![512.0, 550.0])).into(),
            ranges: Rc::new(slint::VecModel::from(vec![
                range(520.0, 80.0, 512.0, 782.0),
                range(780.0, 10.0, 770.0, 890.0),
                range(812.0, 40.0, 782.0, 880.0),
            ]))
            .into(),
            ..Default::default()
        };
        for (target, expected) in [
            (512.0, 512.0),
            (515.0, 520.0),
            (550.0, 520.0),
            (560.0, 520.0),
            (566.0, 600.0),
            (777.0, 760.0),
            (782.0, 760.0),
            (790.0, 812.0),
            (890.0, 890.0),
        ] {
            assert_eq!(
                resolve(
                    config.clone(),
                    request(target, 0.0, ScrollSnapIntent::Nearest)
                ),
                expected
            );
        }
    }

    #[test]
    fn keyboard_moves_past_origin_and_wheel_escapes_only_an_exact_stop() {
        let config = ScrollSnapPoints {
            aligned_points: Rc::new(slint::VecModel::from(vec![0.0, 96.0, 236.0])).into(),
            ..Default::default()
        };
        for (target, origin, intent, expected) in [
            (16.0, 0.0, ScrollSnapIntent::Directional, 96.0),
            (106.0, 96.0, ScrollSnapIntent::Impulse, 236.0),
            (90.0, 80.0, ScrollSnapIntent::Impulse, 96.0),
            (226.0, 236.0, ScrollSnapIntent::Impulse, 96.0),
        ] {
            assert_eq!(
                resolve(config.clone(), request(target, origin, intent)),
                expected
            );
        }
    }
}

use crate::{
    TeachingTipGeometry, TeachingTipLayout, TeachingTipPlacement, TeachingTipPlacementInput,
    TeachingTipSide,
};

const TAIL: f32 = 8.0;
const EDGE_TAIL_CENTER: f32 = 28.0;
const WINDOW_MARGIN: f32 = 24.0;

pub fn install(layout: TeachingTipLayout<'_>) {
    layout.on_place(place);
    layout.set_ready(true);
}

fn even(value: f32) -> f32 {
    (value / 2.0).round() * 2.0
}

fn mirror(p: TeachingTipPlacement) -> TeachingTipPlacement {
    use TeachingTipPlacement::*;
    match p {
        Left => Right,
        Right => Left,
        TopLeft => TopRight,
        TopRight => TopLeft,
        BottomLeft => BottomRight,
        BottomRight => BottomLeft,
        LeftTop => RightTop,
        LeftBottom => RightBottom,
        RightTop => LeftTop,
        RightBottom => LeftBottom,
        _ => p,
    }
}

fn side(p: TeachingTipPlacement) -> TeachingTipSide {
    use TeachingTipPlacement::*;
    match p {
        Auto | Top | TopLeft | TopRight | Center => TeachingTipSide::Top,
        Bottom | BottomLeft | BottomRight => TeachingTipSide::Bottom,
        Left | LeftTop | LeftBottom => TeachingTipSide::Left,
        Right | RightTop | RightBottom => TeachingTipSide::Right,
    }
}

fn fallback_order(p: TeachingTipPlacement) -> [TeachingTipPlacement; 13] {
    use TeachingTipPlacement::*;
    let mut order = [
        Top,
        Bottom,
        Left,
        Right,
        TopLeft,
        TopRight,
        BottomLeft,
        BottomRight,
        LeftTop,
        LeftBottom,
        RightTop,
        RightBottom,
        Center,
    ];
    match side(p) {
        TeachingTipSide::Bottom => {
            order.swap(0, 1);
            order.swap(4, 6);
            order.swap(5, 7);
        }
        TeachingTipSide::Left | TeachingTipSide::Right => {
            for (a, b) in [(0, 2), (1, 3), (4, 8), (5, 9), (6, 10), (7, 11)] {
                order.swap(a, b);
            }
            if side(p) == TeachingTipSide::Right {
                for (a, b) in [(0, 1), (4, 6), (5, 7)] {
                    order.swap(a, b);
                }
            }
        }
        _ => {}
    }
    if let Some(index) = order.iter().position(|mode| *mode == p) {
        order[..=index].rotate_right(1);
    }
    order
}

fn place(input: TeachingTipPlacementInput) -> TeachingTipGeometry {
    use TeachingTipPlacement::*;
    let p = if input.rtl {
        mirror(input.preferred)
    } else {
        input.preferred
    };
    if !input.targeted {
        let near_x = WINDOW_MARGIN + input.left;
        let far_x = input.window_width - input.width - WINDOW_MARGIN - input.right;
        let near_y = WINDOW_MARGIN + input.top;
        let far_y = input.window_height - input.height - WINDOW_MARGIN - input.bottom;
        let x = match p {
            Left | TopLeft | BottomLeft | LeftTop | LeftBottom => near_x,
            Right | TopRight | BottomRight | RightTop | RightBottom => far_x,
            _ => (near_x + far_x) / 2.0,
        };
        let y = match p {
            Top | TopLeft | TopRight | LeftTop | RightTop => near_y,
            Auto | Bottom | BottomLeft | BottomRight | LeftBottom | RightBottom => far_y,
            _ => (near_y + far_y) / 2.0,
        };
        return TeachingTipGeometry {
            x: even(x.clamp(0.0, (input.window_width - input.width).max(0.0))),
            y: even(y.clamp(0.0, (input.window_height - input.height).max(0.0))),
            fits: input.width > 0.0 && input.height > 0.0,
            ..Default::default()
        };
    }

    for mode in fallback_order(p) {
        let candidate = targeted(&input, mode);
        if candidate.fits {
            return candidate;
        }
    }
    TeachingTipGeometry::default()
}

fn targeted(i: &TeachingTipPlacementInput, p: TeachingTipPlacement) -> TeachingTipGeometry {
    use TeachingTipPlacement::*;
    let side = side(p);
    let tail = if i.show_tail { TAIL } else { 0.0 };
    let center_x = i.target.x + i.target.width / 2.0;
    let center_y = i.target.y + i.target.height / 2.0;
    let vertical = matches!(side, TeachingTipSide::Top | TeachingTipSide::Bottom);
    let span = if vertical { i.width } else { i.height };
    let corner = EDGE_TAIL_CENTER.min(span / 2.0);
    let offset = match p {
        TopRight | BottomRight | LeftBottom | RightBottom => corner,
        TopLeft | BottomLeft | LeftTop | RightTop => span - corner,
        _ => span / 2.0,
    };
    let (x, y) = match side {
        TeachingTipSide::Top => (
            center_x - offset,
            (if p == Center { center_y } else { i.target.y }) - i.height - tail - i.top,
        ),
        TeachingTipSide::Bottom => (
            center_x - offset,
            i.target.y + i.target.height + tail + i.bottom,
        ),
        TeachingTipSide::Left => (i.target.x - i.width - tail - i.left, center_y - offset),
        TeachingTipSide::Right => (
            i.target.x + i.target.width + tail + i.right,
            center_y - offset,
        ),
    };
    let x = even(x);
    let y = even(y);
    let left = x - if side == TeachingTipSide::Right {
        tail
    } else {
        0.0
    };
    let top = y - if side == TeachingTipSide::Bottom {
        tail
    } else {
        0.0
    };
    let right = x
        + i.width
        + if side == TeachingTipSide::Left {
            tail
        } else {
            0.0
        };
    let bottom = y
        + i.height
        + if side == TeachingTipSide::Top {
            tail
        } else {
            0.0
        };
    TeachingTipGeometry {
        x,
        y,
        side,
        tail_center: even(if vertical { center_x - x } else { center_y - y }),
        fits: i.width > 0.0
            && i.height > 0.0
            && left >= 0.0
            && top >= 0.0
            && right <= i.window_width
            && bottom <= i.window_height,
        hero_on_bottom: matches!(
            p,
            Bottom | BottomLeft | BottomRight | LeftBottom | RightBottom
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FlyoutRect;

    #[test]
    fn aligns_corner_tails_and_mirrors_rtl() {
        let input = TeachingTipPlacementInput {
            preferred: TeachingTipPlacement::TopRight,
            window_width: 1000.0,
            window_height: 600.0,
            width: 312.0,
            height: 130.0,
            targeted: true,
            show_tail: true,
            target: FlyoutRect {
                x: 400.0,
                y: 250.0,
                width: 80.0,
                height: 32.0,
            },
            ..Default::default()
        };
        let ltr = place(input.clone());
        assert!(ltr.fits);
        assert_eq!((ltr.x, ltr.y, ltr.tail_center), (412.0, 112.0, 28.0));
        let rtl = place(TeachingTipPlacementInput { rtl: true, ..input });
        assert_eq!((rtl.x, rtl.tail_center), (156.0, 284.0));
    }

    #[test]
    fn near_window_edge_flips_top_to_bottom() {
        let result = place(TeachingTipPlacementInput {
            preferred: TeachingTipPlacement::Top,
            window_width: 640.0,
            window_height: 360.0,
            width: 312.0,
            height: 120.0,
            targeted: true,
            show_tail: true,
            target: FlyoutRect {
                x: 290.0,
                y: 20.0,
                width: 60.0,
                height: 32.0,
            },
            ..Default::default()
        });
        assert!(result.fits);
        assert_eq!(
            (result.x, result.y, result.side),
            (164.0, 60.0, TeachingTipSide::Bottom)
        );
        assert!(result.hero_on_bottom);
    }
}

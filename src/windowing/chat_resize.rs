//! Geometry shared by the chat window's eight resize handles. All coordinates
//! in a drag are physical pixels, so moving/resizing does not mix DPI units.
use gpui_kit::ResizeEdge;

pub const DEFAULT_SIZE: (f32, f32) = (420., 560.);
pub const MIN_SIZE: (f32, f32) = (320., 260.);

pub fn restored_size(saved: Option<(f32, f32)>, available: (f32, f32)) -> (f32, f32) {
    let saved = saved
        .filter(|(w, h)| w.is_finite() && h.is_finite() && *w > 0. && *h > 0.)
        .unwrap_or(DEFAULT_SIZE);
    (
        saved.0.max(MIN_SIZE.0).min(available.0.max(1.)),
        saved.1.max(MIN_SIZE.1).min(available.1.max(1.)),
    )
}

#[derive(Clone, Copy)]
pub struct Drag {
    pub edge: ResizeEdge,
    pub pointer: (f32, f32),
    pub bounds: (f32, f32, f32, f32),
    pub minimum: (f32, f32),
}

impl Drag {
    pub fn bounds_at(self, cursor: (f32, f32)) -> (f32, f32, f32, f32) {
        use ResizeEdge::*;
        let (mut left, mut top, mut right, mut bottom) = self.bounds;
        let (dx, dy) = (cursor.0 - self.pointer.0, cursor.1 - self.pointer.1);
        if matches!(self.edge, Left | TopLeft | BottomLeft) {
            left = (left + dx).min(right - self.minimum.0);
        }
        if matches!(self.edge, Right | TopRight | BottomRight) {
            right = (right + dx).max(left + self.minimum.0);
        }
        if matches!(self.edge, Top | TopLeft | TopRight) {
            top = (top + dy).min(bottom - self.minimum.1);
        }
        if matches!(self.edge, Bottom | BottomLeft | BottomRight) {
            bottom = (bottom + dy).max(top + self.minimum.1);
        }
        (left, top, right, bottom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_edge_preserves_the_opposite_edges() {
        use ResizeEdge::*;
        for (edge, expected) in [
            (Left, (125., 200., 520., 760.)),
            (Right, (100., 200., 545., 760.)),
            (Top, (100., 240., 520., 760.)),
            (Bottom, (100., 200., 520., 800.)),
            (TopLeft, (125., 240., 520., 760.)),
            (TopRight, (100., 240., 545., 760.)),
            (BottomLeft, (125., 200., 520., 800.)),
            (BottomRight, (100., 200., 545., 800.)),
        ] {
            let drag = Drag {
                edge,
                pointer: (10., 20.),
                bounds: (100., 200., 520., 760.),
                minimum: MIN_SIZE,
            };
            assert_eq!(drag.bounds_at((35., 60.)), expected, "{edge:?}");
        }
    }

    #[test]
    fn corners_stop_at_minimum_even_past_the_opposite_edge_at_high_dpi() {
        let mut drag = Drag {
            edge: ResizeEdge::TopLeft,
            pointer: (0., 0.),
            bounds: (-800., 100., 40., 1220.),
            minimum: (640., 520.),
        };
        assert_eq!(drag.bounds_at((5000., 5000.)), (-600., 700., 40., 1220.));
        drag.edge = ResizeEdge::BottomRight;
        assert_eq!(drag.bounds_at((-5000., -5000.)), (-800., 100., -160., 620.));
    }

    #[test]
    fn restores_size_without_stranding_it_on_a_smaller_display() {
        assert_eq!(
            restored_size(Some((800., 900.)), (600., 700.)),
            (600., 700.)
        );
        assert_eq!(restored_size(Some((1., 1.)), (1920., 1080.)), MIN_SIZE);
        assert_eq!(
            restored_size(Some((f32::NAN, 1.)), (1920., 1080.)),
            DEFAULT_SIZE
        );
        assert_eq!(restored_size(None, (240., 200.)), (240., 200.));
    }
}

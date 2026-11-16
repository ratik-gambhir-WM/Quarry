//! OOXML geometry and nested group transforms.

use super::super::super::Element;
use super::{emu_attribute_option, is_true, EMUS_PER_PIXEL};
use roxmltree::Node;

#[derive(Clone, Copy)]
pub(super) struct Geometry {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) w: f64,
    pub(super) h: f64,
    pub(super) rotate: f64,
    pub(super) flip_h: bool,
    pub(super) flip_v: bool,
}

#[derive(Clone, Copy)]
pub(super) struct GroupTransform {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    child_x: f64,
    child_y: f64,
    scale_x: f64,
    scale_y: f64,
    rotate: f64,
    flip_h: bool,
    flip_v: bool,
}

impl GroupTransform {
    pub(super) fn from_node(node: Node<'_, '_>) -> Self {
        let xfrm = node
            .children()
            .find(|child| child.tag_name().name() == "grpSpPr")
            .and_then(|properties| {
                properties
                    .children()
                    .find(|child| child.tag_name().name() == "xfrm")
            });
        let child = |name| {
            xfrm.and_then(|transform| {
                transform
                    .children()
                    .find(|child| child.tag_name().name() == name)
            })
        };
        let off = child("off");
        let ext = child("ext");
        let child_off = child("chOff");
        let child_ext = child("chExt");
        let w = emu_attribute_option(ext, "cx") / EMUS_PER_PIXEL;
        let h = emu_attribute_option(ext, "cy") / EMUS_PER_PIXEL;
        let child_w = emu_attribute_option(child_ext, "cx") / EMUS_PER_PIXEL;
        let child_h = emu_attribute_option(child_ext, "cy") / EMUS_PER_PIXEL;
        Self {
            x: emu_attribute_option(off, "x") / EMUS_PER_PIXEL,
            y: emu_attribute_option(off, "y") / EMUS_PER_PIXEL,
            w,
            h,
            child_x: emu_attribute_option(child_off, "x") / EMUS_PER_PIXEL,
            child_y: emu_attribute_option(child_off, "y") / EMUS_PER_PIXEL,
            scale_x: if w.abs() < f64::EPSILON || child_w.abs() < f64::EPSILON {
                1.0
            } else {
                w / child_w
            },
            scale_y: if h.abs() < f64::EPSILON || child_h.abs() < f64::EPSILON {
                1.0
            } else {
                h / child_h
            },
            rotate: xfrm
                .and_then(|transform| transform.attribute("rot"))
                .and_then(|value| value.parse::<f64>().ok())
                .map_or(0.0, |value| value / 60_000.0),
            flip_h: xfrm
                .and_then(|transform| transform.attribute("flipH"))
                .is_some_and(is_true),
            flip_v: xfrm
                .and_then(|transform| transform.attribute("flipV"))
                .is_some_and(is_true),
        }
    }

    pub(super) fn matrix(self) -> TransformMatrix {
        let (e, f) = self.apply_point(0.0, 0.0);
        let (x, y) = self.apply_point(1.0, 0.0);
        let (x2, y2) = self.apply_point(0.0, 1.0);
        TransformMatrix {
            a: x - e,
            b: y - f,
            c: x2 - e,
            d: y2 - f,
            e,
            f,
        }
    }

    pub(super) fn apply_point(self, x: f64, y: f64) -> (f64, f64) {
        let mut x = self.x + (x - self.child_x) * self.scale_x;
        let mut y = self.y + (y - self.child_y) * self.scale_y;
        if self.flip_h {
            x = self.x + self.w - (x - self.x);
        }
        if self.flip_v {
            y = self.y + self.h - (y - self.y);
        }
        if self.rotate.abs() < f64::EPSILON {
            return (x, y);
        }

        let radians = self.rotate.to_radians();
        let (sin, cos) = radians.sin_cos();
        let center_x = self.x + self.w / 2.0;
        let center_y = self.y + self.h / 2.0;
        let offset_x = x - center_x;
        let offset_y = y - center_y;
        x = center_x + offset_x * cos - offset_y * sin;
        y = center_y + offset_x * sin + offset_y * cos;
        (x, y)
    }
}

#[derive(Clone, Copy)]
pub(super) struct TransformMatrix {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl TransformMatrix {
    pub(super) const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    pub(super) fn compose(self, child: Self) -> Self {
        Self {
            a: self.a * child.a + self.c * child.b,
            b: self.b * child.a + self.d * child.b,
            c: self.a * child.c + self.c * child.d,
            d: self.b * child.c + self.d * child.d,
            e: self.a * child.e + self.c * child.f + self.e,
            f: self.b * child.e + self.d * child.f + self.f,
        }
    }

    pub(super) fn apply_rect(self, geometry: &mut Geometry) {
        let center_x = geometry.x + geometry.w / 2.0;
        let center_y = geometry.y + geometry.h / 2.0;
        let width = self.a.hypot(self.b) * geometry.w;
        let height = self.c.hypot(self.d) * geometry.h;
        geometry.x = self.a * center_x + self.c * center_y + self.e - width / 2.0;
        geometry.y = self.b * center_x + self.d * center_y + self.f - height / 2.0;
        geometry.w = width.abs();
        geometry.h = height.abs();
        geometry.rotate = (geometry.rotate + self.b.atan2(self.a).to_degrees()).rem_euclid(360.0);
        geometry.flip_v ^= self.a * self.d - self.b * self.c < 0.0;
    }

    pub(super) fn apply(self, element: &mut Element) {
        let mut value = match &*element {
            Element::Text(value) => Geometry {
                x: value.x,
                y: value.y,
                w: value.w,
                h: value.h,
                rotate: value.rotate,
                flip_h: value.flip_h,
                flip_v: value.flip_v,
            },
            Element::Shape(value) => Geometry {
                x: value.x,
                y: value.y,
                w: value.w,
                h: value.h,
                rotate: value.rotate,
                flip_h: value.flip_h,
                flip_v: value.flip_v,
            },
            Element::Image(value) => Geometry {
                x: value.x,
                y: value.y,
                w: value.w,
                h: value.h,
                rotate: value.rotate,
                flip_h: value.flip_h,
                flip_v: value.flip_v,
            },
            Element::Line(value) => Geometry {
                x: value.x1.min(value.x2),
                y: value.y1.min(value.y2),
                w: (value.x2 - value.x1).abs(),
                h: (value.y2 - value.y1).abs(),
                rotate: value.rotate,
                flip_h: value.x2 < value.x1,
                flip_v: value.y2 < value.y1,
            },
        };
        let min_dimension = value.w.min(value.h);
        self.apply_rect(&mut value);
        let radius_scale = if min_dimension > 0.0 {
            value.w.min(value.h) / min_dimension
        } else {
            1.0
        };
        match element {
            Element::Text(text) => {
                text.border_radius *= radius_scale;
                (text.x, text.y, text.w, text.h) = (value.x, value.y, value.w, value.h);
                (text.rotate, text.flip_h, text.flip_v) =
                    (value.rotate, value.flip_h, value.flip_v);
            }
            Element::Shape(shape) => {
                shape.border_radius *= radius_scale;
                (shape.x, shape.y, shape.w, shape.h) = (value.x, value.y, value.w, value.h);
                (shape.rotate, shape.flip_h, shape.flip_v) =
                    (value.rotate, value.flip_h, value.flip_v);
            }
            Element::Image(image) => {
                (image.x, image.y, image.w, image.h) = (value.x, value.y, value.w, value.h);
                (image.rotate, image.flip_h, image.flip_v) =
                    (value.rotate, value.flip_h, value.flip_v);
            }
            Element::Line(line) => {
                (line.x1, line.x2) = if value.flip_h {
                    (value.x + value.w, value.x)
                } else {
                    (value.x, value.x + value.w)
                };
                (line.y1, line.y2) = if value.flip_v {
                    (value.y + value.h, value.y)
                } else {
                    (value.y, value.y + value.h)
                };
                line.rotate = value.rotate;
            }
        }
    }
}

pub(super) fn geometry(node: Node<'_, '_>) -> Geometry {
    let xfrm = node
        .descendants()
        .find(|child| child.tag_name().name() == "xfrm");
    let off = xfrm.and_then(|node| {
        node.children()
            .find(|child| child.tag_name().name() == "off")
    });
    let ext = xfrm.and_then(|node| {
        node.children()
            .find(|child| child.tag_name().name() == "ext")
    });
    Geometry {
        x: emu_attribute_option(off, "x") / EMUS_PER_PIXEL,
        y: emu_attribute_option(off, "y") / EMUS_PER_PIXEL,
        w: emu_attribute_option(ext, "cx") / EMUS_PER_PIXEL,
        h: emu_attribute_option(ext, "cy") / EMUS_PER_PIXEL,
        rotate: xfrm
            .and_then(|node| node.attribute("rot"))
            .and_then(|value| value.parse::<f64>().ok())
            .map_or(0.0, |value| (value / 60_000.0).rem_euclid(360.0)),
        flip_h: xfrm
            .and_then(|node| node.attribute("flipH"))
            .is_some_and(is_true),
        flip_v: xfrm
            .and_then(|node| node.attribute("flipV"))
            .is_some_and(is_true),
    }
}

//! Image layers — port of `types/editor.ts` `ImageLayer`,
//! `renderer/utils/layer-layout.ts` and `scaleLayerToEdge`. A capture can have
//! further images attached to its edges, laid out around it with the
//! wallpaper's spacing between them.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    #[serde(rename = "self")]
    Primary,
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    #[allow(dead_code)]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "top" => Some(Self::Top),
            "bottom" => Some(Self::Bottom),
            "self" => Some(Self::Primary),
            _ => None,
        }
    }

    fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageLayer {
    pub id: String,
    /// A file path or a `data:` URL, as the renderer stores it.
    pub image_url: String,
    pub natural_width: f64,
    pub natural_height: f64,
    pub edge: Edge,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// The primary image's rect, then one per attached layer in order.
    pub primary: LayerRect,
    pub layers: Vec<(String, LayerRect)>,
    pub width: f64,
    pub height: f64,
}

/// `scaleLayerToEdge` — a side layer matches the anchor's height, a top or
/// bottom layer matches its width.
pub fn scale_to_edge(
    anchor_width: f64,
    anchor_height: f64,
    layer_width: f64,
    layer_height: f64,
    edge: Edge,
) -> (f64, f64) {
    if layer_width <= 0.0 || layer_height <= 0.0 {
        return (0.0, 0.0);
    }
    if edge.is_horizontal() {
        let scale = anchor_height / layer_height;
        return ((layer_width * scale).round(), anchor_height);
    }
    let scale = anchor_width / layer_width;
    (anchor_width, (layer_height * scale).round())
}

/// `computeLayerLayout` — places every layer around the primary image and
/// shifts the result so the top-left corner is the origin.
pub fn compute(
    primary_width: f64,
    primary_height: f64,
    layers: &[ImageLayer],
    spacing: f64,
) -> Layout {
    let mut rects: Vec<(String, LayerRect)> = Vec::new();
    let primary = LayerRect {
        x: 0.0,
        y: 0.0,
        width: primary_width,
        height: primary_height,
    };

    // Each edge stacks outwards, so a second layer on the same edge sits
    // beyond the first.
    let mut offsets = [0.0_f64; 4];
    let slot = |edge: Edge| -> usize {
        match edge {
            Edge::Left => 0,
            Edge::Right => 1,
            Edge::Top => 2,
            Edge::Bottom => 3,
            Edge::Primary => 0,
        }
    };

    for layer in layers {
        if layer.edge == Edge::Primary {
            continue;
        }
        let (width, height) = scale_to_edge(
            primary.width,
            primary.height,
            layer.natural_width,
            layer.natural_height,
            layer.edge,
        );
        if width <= 0.0 || height <= 0.0 {
            continue;
        }
        let index = slot(layer.edge);
        let offset = offsets[index];
        let (x, y) = match layer.edge {
            Edge::Right => (primary.x + primary.width + spacing + offset, primary.y),
            Edge::Left => (primary.x - spacing - offset - width, primary.y),
            Edge::Bottom => (primary.x, primary.y + primary.height + spacing + offset),
            Edge::Top => (primary.x, primary.y - spacing - offset - height),
            Edge::Primary => (primary.x, primary.y),
        };
        offsets[index] = offset
            + spacing
            + if layer.edge.is_horizontal() {
                width
            } else {
                height
            };

        rects.push((
            layer.id.clone(),
            LayerRect {
                x,
                y,
                width,
                height,
            },
        ));
    }

    let mut min_x = primary.x;
    let mut min_y = primary.y;
    let mut max_x = primary.x + primary.width;
    let mut max_y = primary.y + primary.height;
    for (_, rect) in &rects {
        min_x = min_x.min(rect.x);
        min_y = min_y.min(rect.y);
        max_x = max_x.max(rect.x + rect.width);
        max_y = max_y.max(rect.y + rect.height);
    }

    let shift = |rect: LayerRect| LayerRect {
        x: rect.x - min_x,
        y: rect.y - min_y,
        ..rect
    };

    Layout {
        primary: shift(primary),
        layers: rects
            .into_iter()
            .map(|(id, rect)| (id, shift(rect)))
            .collect(),
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(id: &str, width: f64, height: f64, edge: Edge) -> ImageLayer {
        ImageLayer {
            id: id.into(),
            image_url: String::new(),
            natural_width: width,
            natural_height: height,
            edge,
        }
    }

    #[test]
    fn a_side_layer_matches_the_anchors_height() {
        assert_eq!(
            scale_to_edge(200.0, 100.0, 100.0, 50.0, Edge::Right),
            (200.0, 100.0)
        );
        assert_eq!(
            scale_to_edge(200.0, 100.0, 50.0, 100.0, Edge::Left),
            (50.0, 100.0)
        );
    }

    #[test]
    fn a_stacked_layer_matches_the_anchors_width() {
        assert_eq!(
            scale_to_edge(200.0, 100.0, 100.0, 50.0, Edge::Top),
            (200.0, 100.0)
        );
        assert_eq!(
            scale_to_edge(200.0, 100.0, 400.0, 100.0, Edge::Bottom),
            (200.0, 50.0)
        );
    }

    #[test]
    fn no_layers_leaves_the_primary_at_the_origin() {
        let layout = compute(200.0, 100.0, &[], 10.0);
        assert_eq!(
            layout.primary,
            LayerRect {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 100.0
            }
        );
        assert_eq!((layout.width, layout.height), (200.0, 100.0));
        assert!(layout.layers.is_empty());
    }

    #[test]
    fn a_right_layer_extends_the_canvas_and_leaves_the_primary_put() {
        let layout = compute(200.0, 100.0, &[layer("a", 100.0, 100.0, Edge::Right)], 10.0);
        assert_eq!(layout.primary.x, 0.0);
        assert_eq!(layout.layers[0].1.x, 210.0);
        assert_eq!(layout.width, 310.0);
        assert_eq!(layout.height, 100.0);
    }

    #[test]
    fn a_left_layer_shifts_the_primary_right() {
        let layout = compute(200.0, 100.0, &[layer("a", 100.0, 100.0, Edge::Left)], 10.0);
        assert_eq!(layout.layers[0].1.x, 0.0);
        assert_eq!(layout.primary.x, 110.0);
        assert_eq!(layout.width, 310.0);
    }

    #[test]
    fn layers_on_the_same_edge_stack_outwards() {
        let layout = compute(
            100.0,
            100.0,
            &[
                layer("a", 100.0, 100.0, Edge::Bottom),
                layer("b", 100.0, 100.0, Edge::Bottom),
            ],
            10.0,
        );
        assert_eq!(layout.layers[0].1.y, 110.0);
        assert_eq!(layout.layers[1].1.y, 220.0);
        assert_eq!(layout.height, 320.0);
    }

    #[test]
    fn a_self_layer_is_ignored() {
        let layout = compute(100.0, 100.0, &[layer("a", 50.0, 50.0, Edge::Primary)], 10.0);
        assert!(layout.layers.is_empty());
    }

    #[test]
    fn edges_parse_from_the_persisted_names() {
        assert_eq!(Edge::parse("left"), Some(Edge::Left));
        assert_eq!(Edge::parse("self"), Some(Edge::Primary));
        assert_eq!(Edge::parse("diagonal"), None);
    }

    #[test]
    fn a_layer_serializes_with_the_renderer_key_names() {
        let json = serde_json::to_value(layer("a", 10.0, 20.0, Edge::Top)).expect("layer");
        assert_eq!(json["naturalWidth"], 10.0);
        assert_eq!(json["edge"], "top");
        assert_eq!(
            serde_json::to_value(layer("a", 1.0, 1.0, Edge::Primary)).expect("layer")["edge"],
            "self"
        );
    }
}

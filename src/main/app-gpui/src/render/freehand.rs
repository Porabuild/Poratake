//! Port of `perfect-freehand` 1.2.3 (`getStrokePoints` +
//! `getStrokeOutlinePoints`), the pressure-simulating stroke outliner the
//! renderer draws pen annotations with. Both editors call it with the same
//! options — `thinning 0.5`, `smoothing 0.6`, `streamline 0.5`, simulated
//! pressure, round caps, no taper — so those are the options modelled here.

pub type Vec2 = (f64, f64);

const RATE_OF_PRESSURE_CHANGE: f64 = 0.275;
/// PI with a tiny offset, which the upstream algorithm uses to avoid
/// degenerate rotations.
const FIXED_PI: f64 = std::f64::consts::PI + 0.0001;
const START_CAP_SEGMENTS: usize = 13;
const END_CAP_SEGMENTS: usize = 29;
const CORNER_CAP_SEGMENTS: usize = 13;
const END_NOISE_THRESHOLD: f64 = 3.0;
const MIN_STREAMLINE_T: f64 = 0.15;
const STREAMLINE_T_RANGE: f64 = 0.85;
const MIN_RADIUS: f64 = 0.01;
const DEFAULT_FIRST_PRESSURE: f64 = 0.25;
const DEFAULT_PRESSURE: f64 = 0.5;
const UNIT_OFFSET: Vec2 = (1.0, 1.0);

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub size: f64,
    pub thinning: f64,
    pub smoothing: f64,
    pub streamline: f64,
    pub simulate_pressure: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            size: 16.0,
            thinning: 0.5,
            smoothing: 0.6,
            streamline: 0.5,
            simulate_pressure: true,
        }
    }
}

impl Options {
    /// The options both editors pass, differing only in stroke size.
    pub fn for_pen(stroke_width: f64) -> Self {
        Self {
            size: stroke_width * 2.0,
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct StrokePoint {
    point: Vec2,
    pressure: f64,
    vector: Vec2,
    distance: f64,
    running_length: f64,
}

fn neg(a: Vec2) -> Vec2 {
    (-a.0, -a.1)
}

fn add(a: Vec2, b: Vec2) -> Vec2 {
    (a.0 + b.0, a.1 + b.1)
}

fn sub(a: Vec2, b: Vec2) -> Vec2 {
    (a.0 - b.0, a.1 - b.1)
}

fn mul(a: Vec2, n: f64) -> Vec2 {
    (a.0 * n, a.1 * n)
}

fn per(a: Vec2) -> Vec2 {
    (a.1, -a.0)
}

fn dpr(a: Vec2, b: Vec2) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

fn len(a: Vec2) -> f64 {
    a.0.hypot(a.1)
}

fn uni(a: Vec2) -> Vec2 {
    let length = len(a);
    if length == 0.0 {
        return (0.0, 0.0);
    }
    (a.0 / length, a.1 / length)
}

fn dist(a: Vec2, b: Vec2) -> f64 {
    (a.1 - b.1).hypot(a.0 - b.0)
}

fn dist2(a: Vec2, b: Vec2) -> f64 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    dx * dx + dy * dy
}

fn lrp(a: Vec2, b: Vec2, t: f64) -> Vec2 {
    add(a, mul(sub(b, a), t))
}

fn prj(a: Vec2, b: Vec2, c: f64) -> Vec2 {
    add(a, mul(b, c))
}

fn rot_around(a: Vec2, center: Vec2, radians: f64) -> Vec2 {
    let (sin, cos) = radians.sin_cos();
    let px = a.0 - center.0;
    let py = a.1 - center.1;
    (
        px * cos - py * sin + center.0,
        px * sin + py * cos + center.1,
    )
}

fn simulate_pressure(previous: f64, distance: f64, size: f64) -> f64 {
    let speed = (distance / size).min(1.0);
    let rate = (1.0 - speed).min(1.0);
    (previous + (rate - previous) * (speed * RATE_OF_PRESSURE_CHANGE)).min(1.0)
}

fn stroke_radius(size: f64, thinning: f64, pressure: f64) -> f64 {
    size * (0.5 - thinning * (0.5 - pressure))
}

fn stroke_points(points: &[Vec2], options: &Options) -> Vec<StrokePoint> {
    if points.is_empty() {
        return Vec::new();
    }
    let t = MIN_STREAMLINE_T + (1.0 - options.streamline) * STREAMLINE_T_RANGE;

    let mut pts: Vec<Vec2> = points.to_vec();
    if pts.len() == 2 {
        // Extra points between the two avoid a "dash" for a two-point stroke.
        let last = pts[1];
        let first = pts[0];
        pts.truncate(1);
        for index in 1..5 {
            pts.push(lrp(first, last, index as f64 / 4.0));
        }
    }
    if pts.len() == 1 {
        pts.push(add(pts[0], UNIT_OFFSET));
    }

    let mut stroke: Vec<StrokePoint> = vec![StrokePoint {
        point: pts[0],
        pressure: DEFAULT_FIRST_PRESSURE,
        vector: UNIT_OFFSET,
        distance: 0.0,
        running_length: 0.0,
    }];

    let mut has_reached_minimum_length = false;
    let mut running_length = 0.0;
    let mut previous = stroke[0];
    let max = pts.len() - 1;

    for index in 1..pts.len() {
        let point = lrp(previous.point, pts[index], t);
        if point == previous.point {
            continue;
        }

        let distance = dist(point, previous.point);
        running_length += distance;

        // The start of a line is noisy, so wait until it has travelled at
        // least one stroke width before recording points.
        if index < max && !has_reached_minimum_length {
            if running_length < options.size {
                continue;
            }
            has_reached_minimum_length = true;
        }

        previous = StrokePoint {
            point,
            pressure: DEFAULT_PRESSURE,
            vector: uni(sub(previous.point, point)),
            distance,
            running_length,
        };
        stroke.push(previous);
    }

    if let Some(second) = stroke.get(1).map(|point| point.vector) {
        stroke[0].vector = second;
    } else {
        stroke[0].vector = (0.0, 0.0);
    }
    stroke
}

fn draw_dot(center: Vec2, radius: f64) -> Vec<Vec2> {
    let offset_point = add(center, (1.0, 1.0));
    let start = prj(center, uni(per(sub(center, offset_point))), -radius);
    let step = 1.0 / START_CAP_SEGMENTS as f64;
    let mut points = Vec::new();
    let mut t = step;
    while t <= 1.0 {
        points.push(rot_around(start, center, FIXED_PI * 2.0 * t));
        t += step;
    }
    points
}

fn draw_round_start_cap(center: Vec2, right: Vec2) -> Vec<Vec2> {
    let step = 1.0 / START_CAP_SEGMENTS as f64;
    let mut cap = Vec::new();
    let mut t = step;
    while t <= 1.0 {
        cap.push(rot_around(right, center, FIXED_PI * t));
        t += step;
    }
    cap
}

fn draw_round_end_cap(center: Vec2, direction: Vec2, radius: f64) -> Vec<Vec2> {
    let start = prj(center, direction, radius);
    let step = 1.0 / END_CAP_SEGMENTS as f64;
    let mut cap = Vec::new();
    let mut t = step;
    while t < 1.0 {
        cap.push(rot_around(start, center, FIXED_PI * 3.0 * t));
        t += step;
    }
    cap
}

fn initial_pressure(points: &[StrokePoint], simulate: bool, size: f64) -> f64 {
    points
        .iter()
        .take(10)
        .fold(points[0].pressure, |accumulated, current| {
            let pressure = if simulate {
                simulate_pressure(accumulated, current.distance, size)
            } else {
                current.pressure
            };
            (accumulated + pressure) / 2.0
        })
}

fn outline_points(points: &[StrokePoint], options: &Options) -> Vec<Vec2> {
    if points.is_empty() || options.size <= 0.0 {
        return Vec::new();
    }

    let total_length = points[points.len() - 1].running_length;
    let min_distance = (options.size * options.smoothing).powi(2);

    let mut left: Vec<Vec2> = Vec::new();
    let mut right: Vec<Vec2> = Vec::new();

    let mut previous_pressure = initial_pressure(points, options.simulate_pressure, options.size);
    let mut radius = stroke_radius(
        options.size,
        options.thinning,
        points[points.len() - 1].pressure,
    );
    let mut first_radius: Option<f64> = None;
    let mut previous_vector = points[0].vector;
    let mut previous_left = points[0].point;
    let mut previous_right = previous_left;
    let mut is_previous_point_sharp_corner = false;

    for index in 0..points.len() {
        let StrokePoint {
            point,
            vector,
            distance,
            running_length,
            mut pressure,
        } = points[index];
        let is_last_point = index == points.len() - 1;

        if !is_last_point && total_length - running_length < END_NOISE_THRESHOLD {
            continue;
        }

        if options.thinning != 0.0 {
            if options.simulate_pressure {
                pressure = simulate_pressure(previous_pressure, distance, options.size);
            }
            radius = stroke_radius(options.size, options.thinning, pressure);
        } else {
            radius = options.size / 2.0;
        }
        if first_radius.is_none() {
            first_radius = Some(radius);
        }
        radius = radius.max(MIN_RADIUS);

        let next_vector = if is_last_point {
            vector
        } else {
            points[index + 1].vector
        };
        let next_dpr = if is_last_point {
            1.0
        } else {
            dpr(vector, next_vector)
        };
        let previous_dpr = dpr(vector, previous_vector);

        let is_point_sharp_corner = previous_dpr < 0.0 && !is_previous_point_sharp_corner;
        let is_next_point_sharp_corner = next_dpr < 0.0;

        if is_point_sharp_corner || is_next_point_sharp_corner {
            // A sharp corner gets a rounded cap so the outline does not fold
            // back over itself.
            let offset = mul(per(previous_vector), radius);
            let step = 1.0 / CORNER_CAP_SEGMENTS as f64;
            let mut t = 0.0;
            let mut temporary_left = previous_left;
            let mut temporary_right = previous_right;
            while t <= 1.0 {
                temporary_left = rot_around(sub(point, offset), point, FIXED_PI * t);
                left.push(temporary_left);
                temporary_right = rot_around(add(point, offset), point, FIXED_PI * -t);
                right.push(temporary_right);
                t += step;
            }
            previous_left = temporary_left;
            previous_right = temporary_right;
            if is_next_point_sharp_corner {
                is_previous_point_sharp_corner = true;
            }
            continue;
        }

        is_previous_point_sharp_corner = false;

        if is_last_point {
            let offset = mul(per(vector), radius);
            left.push(sub(point, offset));
            right.push(add(point, offset));
            continue;
        }

        let offset = mul(per(lrp(next_vector, vector, next_dpr)), radius);

        let temporary_left = sub(point, offset);
        if index <= 1 || dist2(previous_left, temporary_left) > min_distance {
            left.push(temporary_left);
            previous_left = temporary_left;
        }

        let temporary_right = add(point, offset);
        if index <= 1 || dist2(previous_right, temporary_right) > min_distance {
            right.push(temporary_right);
            previous_right = temporary_right;
        }

        previous_pressure = pressure;
        previous_vector = vector;
    }

    let first_point = points[0].point;
    let last_point = if points.len() > 1 {
        points[points.len() - 1].point
    } else {
        add(points[0].point, (1.0, 1.0))
    };

    let mut start_cap: Vec<Vec2> = Vec::new();
    let mut end_cap: Vec<Vec2> = Vec::new();

    if points.len() == 1 {
        return draw_dot(first_point, first_radius.unwrap_or(radius));
    }

    if let Some(first_right) = right.first().copied() {
        start_cap.extend(draw_round_start_cap(first_point, first_right));
    }
    let direction = per(neg(points[points.len() - 1].vector));
    end_cap.extend(draw_round_end_cap(last_point, direction, radius));

    // Left side, around the end cap, back along the right side, then the
    // start cap — the winding order the outline is filled with.
    right.reverse();
    let mut outline = left;
    outline.extend(end_cap);
    outline.extend(right);
    outline.extend(start_cap);
    outline
}

/// `getStroke` — the polygon that surrounds `points`.
pub fn stroke(points: &[Vec2], options: &Options) -> Vec<Vec2> {
    outline_points(&stroke_points(points, options), options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_input_produces_no_outline() {
        assert!(stroke(&[], &Options::for_pen(4.0)).is_empty());
    }

    #[test]
    fn a_single_point_produces_a_closed_dot() {
        let outline = stroke(&[(10.0, 10.0)], &Options::for_pen(4.0));
        assert!(outline.len() >= START_CAP_SEGMENTS);
        let far = outline
            .iter()
            .map(|point| dist(*point, (10.0, 10.0)))
            .fold(0.0_f64, f64::max);
        assert!(far > 0.0 && far < 20.0, "{far}");
    }

    #[test]
    fn a_line_produces_an_outline_that_surrounds_it() {
        let points: Vec<Vec2> = (0..40).map(|step| (step as f64 * 4.0, 50.0)).collect();
        let outline = stroke(&points, &Options::for_pen(6.0));
        assert!(outline.len() > 20);

        let min_x = outline.iter().map(|point| point.0).fold(f64::MAX, f64::min);
        let max_x = outline.iter().map(|point| point.0).fold(f64::MIN, f64::max);
        let min_y = outline.iter().map(|point| point.1).fold(f64::MAX, f64::min);
        let max_y = outline.iter().map(|point| point.1).fold(f64::MIN, f64::max);
        assert!(min_x < 20.0 && max_x > 140.0, "{min_x} {max_x}");
        assert!(min_y < 50.0 && max_y > 50.0, "{min_y} {max_y}");
    }

    #[test]
    fn a_wider_pen_produces_a_thicker_outline() {
        let points: Vec<Vec2> = (0..40).map(|step| (step as f64 * 4.0, 50.0)).collect();
        let thickness = |width: f64| {
            let outline = stroke(&points, &Options::for_pen(width));
            let min = outline.iter().map(|point| point.1).fold(f64::MAX, f64::min);
            let max = outline.iter().map(|point| point.1).fold(f64::MIN, f64::max);
            max - min
        };
        assert!(thickness(10.0) > thickness(3.0));
    }

    #[test]
    fn pressure_simulation_stays_in_range() {
        let mut pressure = 0.5;
        for distance in [0.0, 1.0, 5.0, 40.0] {
            pressure = simulate_pressure(pressure, distance, 8.0);
            assert!((0.0..=1.0).contains(&pressure), "{pressure}");
        }
    }
}

pub const DRAG_THRESHOLD: f32 = 5.0;

fn starts(durations: &[f64]) -> Vec<f64> {
    let mut start = 0.0;
    durations
        .iter()
        .map(|duration| {
            let at = start;
            start += duration;
            at
        })
        .collect()
}

pub fn drop_index(durations: &[f64], dragged: usize, pointer_time: f64) -> usize {
    let starts = starts(durations);
    for (index, (start, duration)) in starts.iter().zip(durations).enumerate() {
        if pointer_time >= start + duration / 2.0 {
            continue;
        }
        if index == dragged {
            return dragged;
        }
        return if index > dragged { index - 1 } else { index };
    }
    durations.len().saturating_sub(1)
}

pub fn drop_indicator_time(durations: &[f64], dragged: usize, drop: usize) -> Option<f64> {
    if dragged == drop {
        return None;
    }
    let starts = starts(durations);
    let start = *starts.get(drop)?;
    if drop < dragged {
        return Some(start);
    }
    Some(start + durations.get(drop)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pointer_past_a_neighbours_midpoint_drops_after_it() {
        let durations = [4.0, 4.0, 4.0];
        assert_eq!(drop_index(&durations, 0, 0.5), 0);
        assert_eq!(drop_index(&durations, 0, 5.0), 0);
        assert_eq!(drop_index(&durations, 0, 7.0), 1);
        assert_eq!(drop_index(&durations, 0, 11.0), 2);
        assert_eq!(drop_index(&durations, 2, 1.0), 0);
        assert_eq!(drop_index(&durations, 2, 100.0), 2);
    }

    #[test]
    fn an_empty_lane_has_nowhere_to_drop() {
        assert_eq!(drop_index(&[], 0, 1.0), 0);
    }

    #[test]
    fn the_indicator_marks_the_edge_the_clip_lands_against() {
        let durations = [4.0, 4.0, 4.0];
        assert_eq!(drop_indicator_time(&durations, 1, 1), None);
        assert_eq!(drop_indicator_time(&durations, 2, 0), Some(0.0));
        assert_eq!(drop_indicator_time(&durations, 0, 2), Some(12.0));
        assert_eq!(drop_indicator_time(&durations, 0, 9), None);
    }
}

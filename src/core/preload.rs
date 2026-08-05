use std::path::{Path, PathBuf};

use crate::core::navigation::Navigation;
pub const PRELOAD_DEPTH: isize = 1;
pub fn preload_targets(
    nav: &Navigation,
    depth: isize,
    is_cached: impl Fn(&Path) -> bool,
    is_in_flight: impl Fn(&Path) -> bool,
) -> Vec<PathBuf> {
    let mut targets = Vec::new();
    let len = nav.images.len() as isize;
    if len == 0 {
        return targets;
    }
    for d in 1..=depth {
        for offset in [d, -d] {
            let idx = (nav.current as isize + offset).rem_euclid(len) as usize;
            if idx == nav.current {
                continue;
            }
            let path = &nav.images[idx];
            if !is_cached(path) && !is_in_flight(path) && !targets.contains(path) {
                targets.push(path.clone());
            }
        }
    }
    targets
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nav(images: &[&str], current: usize) -> Navigation {
        Navigation {
            images: images.iter().map(PathBuf::from).collect(),
            current,
        }
    }

    fn never(_p: &Path) -> bool {
        false
    }

    #[test]
    fn preloads_prev_and_next_in_priority_order() {
        let n = nav(&["0.png", "1.png", "2.png", "3.png"], 2);
        let targets = preload_targets(&n, 1, never, never);
        assert_eq!(
            targets,
            vec![PathBuf::from("3.png"), PathBuf::from("1.png")]
        );
    }

    #[test]
    fn wraps_circularly() {
        let n = nav(&["0.png", "1.png", "2.png"], 0);
        let targets = preload_targets(&n, 1, never, never);
        assert_eq!(
            targets,
            vec![PathBuf::from("1.png"), PathBuf::from("2.png")]
        );
    }

    #[test]
    fn skips_cached_paths() {
        let n = nav(&["0.png", "1.png", "2.png", "3.png"], 1);
        let targets = preload_targets(&n, 1, |p| p == Path::new("2.png"), never);
        assert_eq!(targets, vec![PathBuf::from("0.png")]);
    }

    #[test]
    fn skips_in_flight_paths() {
        let n = nav(&["0.png", "1.png", "2.png", "3.png"], 1);
        let targets = preload_targets(&n, 1, never, |p| p == Path::new("2.png"));
        assert_eq!(targets, vec![PathBuf::from("0.png")]);
    }

    #[test]
    fn depth_two_preloads_four_in_order() {
        let n = nav(&["0.png", "1.png", "2.png", "3.png", "4.png", "5.png"], 3);
        let targets = preload_targets(&n, 2, never, never);
        assert_eq!(
            targets,
            vec![
                PathBuf::from("4.png"),
                PathBuf::from("2.png"),
                PathBuf::from("5.png"),
                PathBuf::from("1.png"),
            ]
        );
    }

    #[test]
    fn skips_current_image() {
        let n = nav(&["0.png"], 0);
        assert!(preload_targets(&n, 3, never, never).is_empty());
    }

    #[test]
    fn no_duplicates_when_depth_exceeds_size() {
        let n = nav(&["0.png", "1.png", "2.png"], 1);
        let targets = preload_targets(&n, 5, never, never);
        assert_eq!(targets.len(), 2);
    }
}

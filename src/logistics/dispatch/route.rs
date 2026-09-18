//! Pure route-ordering math for multi-stop trips: no I/O, no database — just
//! geography, so it's fully unit-testable on its own. Used by
//! [`crate::logistics::orgs::orgs::Organization::dispatch_trip_to_customers`]
//! to approximate a shorter visiting order when a trip opts into
//! `optimize_route`.

/// Great-circle distance between two `(latitude, longitude)` points, in
/// kilometres.
pub fn haversine_distance_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0;
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}

/// Order `points` by greedy nearest-neighbour, starting from `start` and
/// repeatedly hopping to whichever remaining point is closest to wherever the
/// walk currently is. Returns a permutation of `0..points.len()` — the
/// visiting order, indexing into `points` (`start` itself isn't part of the
/// output).
///
/// This is a heuristic, not an exact solution — the general "visit every
/// point once for minimum total distance" problem (TSP) is NP-hard, and nothing
/// here attempts a globally optimal tour. Nearest-neighbour is a standard,
/// fast approximation that works well for the handful of stops a real
/// delivery trip has; it typically lands within ~25% of optimal for such
/// small, geographically clustered inputs.
pub fn nearest_neighbor_order(start: (f64, f64), points: &[(f64, f64)]) -> Vec<usize> {
    let mut remaining: Vec<usize> = (0..points.len()).collect();
    let mut order = Vec::with_capacity(points.len());
    let mut current = start;

    while !remaining.is_empty() {
        let (pos, &chosen) = remaining
            .iter()
            .enumerate()
            .min_by(|&(_, &a), &(_, &b)| {
                let da = haversine_distance_km(current.0, current.1, points[a].0, points[a].1);
                let db = haversine_distance_km(current.0, current.1, points[b].0, points[b].1);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("remaining is non-empty inside the loop");
        current = points[chosen];
        order.push(chosen);
        remaining.remove(pos);
    }

    order
}

/// Total great-circle distance of a route that starts at `start` and visits
/// `points` in the given order. Used to compare an optimized order against
/// the order the caller originally supplied.
pub fn route_distance_km(start: (f64, f64), points_in_order: &[(f64, f64)]) -> f64 {
    let mut total = 0.0;
    let mut current = start;
    for &p in points_in_order {
        total += haversine_distance_km(current.0, current.1, p.0, p.1);
        current = p;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haversine_zero_distance_for_the_same_point() {
        assert_eq!(haversine_distance_km(18.52, 73.85, 18.52, 73.85), 0.0);
    }

    #[test]
    fn haversine_matches_a_known_approximate_distance() {
        // Pune to Mumbai is roughly 120km as the crow flies.
        let d = haversine_distance_km(18.5204, 73.8567, 19.0760, 72.8777);
        assert!((110.0..=135.0).contains(&d), "got {d}");
    }

    #[test]
    fn nearest_neighbor_order_is_identity_when_already_sorted_by_distance() {
        let start = (0.0, 0.0);
        let points = [(0.0, 1.0), (0.0, 2.0), (0.0, 3.0)];
        assert_eq!(nearest_neighbor_order(start, &points), vec![0, 1, 2]);
    }

    #[test]
    fn nearest_neighbor_order_reorders_an_out_of_order_input() {
        let start = (0.0, 0.0);
        // Given far, near, middle — nearest-neighbour should visit near
        // first, then middle, then far.
        let points = [(0.0, 5.0), (0.0, 1.0), (0.0, 3.0)];
        assert_eq!(nearest_neighbor_order(start, &points), vec![1, 2, 0]);
    }

    #[test]
    fn nearest_neighbor_order_handles_empty_and_single_point_input() {
        assert_eq!(nearest_neighbor_order((0.0, 0.0), &[]), Vec::<usize>::new());
        assert_eq!(nearest_neighbor_order((0.0, 0.0), &[(1.0, 1.0)]), vec![0]);
    }

    #[test]
    fn nearest_neighbor_order_is_a_permutation_of_every_index() {
        let start = (18.5, 73.8);
        let points = [(18.55, 73.7), (18.4, 73.9), (18.6, 73.75), (18.45, 73.82)];
        let mut order = nearest_neighbor_order(start, &points);
        order.sort_unstable();
        assert_eq!(order, vec![0, 1, 2, 3]);
    }

    #[test]
    fn nearest_neighbor_order_never_produces_a_longer_route_than_the_input_order_on_a_scattered_case() {
        let start = (18.50, 73.80);
        let points = [(18.70, 73.60), (18.51, 73.81), (18.90, 73.40), (18.52, 73.79)];
        let optimized = nearest_neighbor_order(start, &points);
        let optimized_points: Vec<(f64, f64)> = optimized.iter().map(|&i| points[i]).collect();

        let optimized_dist = route_distance_km(start, &optimized_points);
        let original_dist = route_distance_km(start, &points);
        assert!(
            optimized_dist <= original_dist,
            "optimized {optimized_dist} should not exceed original {original_dist}"
        );
    }
}

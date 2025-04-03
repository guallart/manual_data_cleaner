use std::iter::once;

pub type Point = [f64; 2];

// Given three collinear points p, q, r, the function checks if
// point q lies on line segment 'pr'
fn on_segment(p: &Point, q: &Point, r: &Point) -> bool {
    q[0] <= p[0].max(r[0])
        && q[0] >= p[0].min(r[0])
        && q[1] <= p[1].max(r[1])
        && q[1] >= p[1].min(r[1])
}

#[derive(PartialEq)]
enum Orientation {
    Colinear,
    Clockwise,
    Counterclockwise,
}

fn orientation(p: &Point, q: &Point, r: &Point) -> Orientation {
    let val = (q[1] - p[1]) * (r[0] - q[0]) - (q[0] - p[0]) * (r[1] - q[1]);
    if val > 0.0 {
        Orientation::Clockwise
    } else if val < 0.0 {
        Orientation::Counterclockwise
    } else {
        Orientation::Colinear
    }
}

fn do_intersect(p1: &Point, q1: &Point, p2: &Point, q2: &Point) -> bool {
    let o1 = orientation(p1, q1, p2);
    let o2 = orientation(p1, q1, q2);
    let o3 = orientation(p2, q2, p1);
    let o4 = orientation(p2, q2, q1);

    // General case
    if o1 != o2 && o3 != o4 {
        return true;
    }

    // Special Cases: check collinear points lying on the segment
    matches!(o1, Orientation::Colinear) && on_segment(p1, p2, q1)
        || matches!(o2, Orientation::Colinear) && on_segment(p1, q2, q1)
        || matches!(o3, Orientation::Colinear) && on_segment(p2, p1, q2)
        || matches!(o4, Orientation::Colinear) && on_segment(p2, q1, q2)
}

pub fn check_inside_curve(curve: Vec<Point>, data: Vec<Point>) -> Vec<bool> {
    let outside = [-100.0, -100.0];

    let first_point = curve[0];
    let last_point = curve[curve.len() - 1];
    let close_loop = [last_point, first_point];

    let results = data
        .iter()
        .map(|p| {
            let n_crossings = curve
                .windows(2)
                .chain(once(&close_loop[..]))
                .map(|segment| do_intersect(&outside, p, &segment[0], &segment[1]) as u8)
                .sum::<u8>();

            n_crossings % 2 == 1
        })
        .collect::<Vec<bool>>();

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_on_segment() {
        let p = [0.0, 0.0];
        let q = [1.0, 1.0];
        let r = [2.0, 2.0];
        assert_eq!(on_segment(&p, &q, &r), true);

        let p = [0.0, 0.0];
        let q = [2.0, 2.0];
        let r = [1.0, 1.0];
        assert_eq!(on_segment(&p, &q, &r), false);
    }

    #[test]
    fn test_do_intersect() {
        let p1 = [-1.0, 0.0];
        let q1 = [1.0, 0.0];
        let p2 = [0.0, 1.0];
        let q2 = [0.0, -1.0];
        assert_eq!(do_intersect(&p1, &q1, &p2, &q2), true);

        let p1 = [2.0, 0.0];
        let q1 = [3.0, 0.0];
        let p2 = [0.0, 1.0];
        let q2 = [0.0, -1.0];
        assert_eq!(do_intersect(&p1, &q1, &p2, &q2), false);

        let p1 = [0.0, 0.0];
        let q1 = [3.0, 0.0];
        let p2 = [0.0, 1.0];
        let q2 = [0.0, -1.0];
        assert_eq!(do_intersect(&p1, &q1, &p2, &q2), true);

        let p1 = [0.0, 0.0];
        let q1 = [3.0, 0.0];
        let p2 = [0.0, 0.0];
        let q2 = [0.0, -1.0];
        assert_eq!(do_intersect(&p1, &q1, &p2, &q2), true);
    }
}

use std::rc::Rc;

use crate::data::tight_poly::TightPoly;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    dx: i32,
    dy: i32,
    poly: Rc<TightPoly>,
}

impl Placement {
    pub fn new(dx: i32, dy: i32, poly: Rc<TightPoly>) -> Placement {
        Placement { dx, dy, poly }
    }

    pub fn intersects(&self, other: &Placement) -> bool {
        self.poly
            .intersects(&other.poly, other.dx - self.dx, other.dy - self.dy)
    }

    pub fn get(&self, x: u32, y: u32) -> bool {
        x as i32 >= self.dx
            && y as i32 >= self.dy
            && self
                .poly
                .get((x as i32 - self.dx) as usize, (y as i32 - self.dy) as usize)
    }
}

#[cfg(test)]
mod tests {
    use std::{rc::Rc, str::FromStr};

    use crate::data::tight_poly::TightPoly;

    use super::Placement;

    #[test]
    fn test_intersects() {
        let f_pentomino: Rc<TightPoly> = TightPoly::from_str(
            r#"
3 3
.##
##.
.#.
"#,
        )
        .unwrap()
        .into();
        let u_pentomino: Rc<TightPoly> = TightPoly::from_str(
            r#"
3 2
##
#.
##
"#,
        )
        .unwrap()
        .into();

        for (dx1, dy1, dx2, dy2, want) in [
            (0, 0, 0, 0, true),
            (0, -1, 0, 0, true),
            (0, 0, 0, -1, false),
            (0, 0, -1, 2, true),
            (-1, 2, 0, 0, false),
        ] {
            assert_eq!(
                Placement::new(dx1, dy1, f_pentomino.clone()).intersects(&Placement::new(
                    dx2,
                    dy2,
                    u_pentomino.clone()
                )),
                want,
                "{dx1} {dy1} {dx2} {dy2} {want}"
            );
        }
    }

    #[test]
    fn test_get() {
        let f_pentomino: Rc<TightPoly> = TightPoly::from_str(
            r#"
3 3
##.
.##
.#.
"#,
        )
        .unwrap()
        .into();

        let placement = Placement::new(1, 1, f_pentomino.clone());

        assert!(!placement.get(0, 0));
        assert!(placement.get(1, 1));
        assert!(placement.get(2, 2));
        assert!(placement.get(2, 3));
        assert!(!placement.get(3, 3));
    }
}

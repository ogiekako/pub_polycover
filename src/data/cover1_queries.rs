use std::collections::BTreeSet;

use super::{tight_poly::TightPoly, vector::Vector};

#[derive(Clone)]
pub struct Cover1Queries {
    problem_cells: Vec<Vector>,
    full_mask: usize,
    side: usize,
    mask: Vec<usize>,
    cover1: BTreeSet<Vector>,
}

impl Cover1Queries {
    pub fn new(problem: TightPoly, side: usize) -> Self {
        let problem_cells = problem.cells();
        let full_mask = (1 << problem_cells.len()) - 1;
        let mask = vec![0; side * side];
        let cover1 = BTreeSet::new();
        Self {
            problem_cells,
            full_mask,
            side,
            mask,
            cover1,
        }
    }

    pub fn flip(&mut self, p: Vector) {
        for (i, q) in self.problem_cells.iter().enumerate() {
            if p.x < q.x || p.y < q.y {
                continue;
            }

            let r = p - q;

            let m = &mut self.mask[r.x as usize * self.side + r.y as usize];

            if *m == self.full_mask {
                self.cover1.remove(&-r);
            }

            *m ^= 1 << i;

            if *m == self.full_mask {
                self.cover1.insert(-r);
            }
        }
    }

    pub fn coverings1(&self) -> &BTreeSet<Vector> {
        &self.cover1
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::data::tight_poly::TightPoly;

    #[test]
    fn test_coverings1() {
        // ###
        // #..
        let problem = TightPoly::from_str("2 3\n###\n#..").unwrap();
        let mut board = Cover1Queries::new(problem, 5);

        assert_eq!(board.coverings1().len(), 0);

        board.flip((2, 1).into());
        board.flip((2, 2).into());
        board.flip((2, 3).into());
        board.flip((3, 1).into());

        // ###
        // #..
        assert_eq!(
            board.coverings1().iter().copied().collect::<Vec<_>>(),
            vec![-Vector::new(2, 1)]
        );

        board.flip((2, 3).into());

        // ##
        // #.
        assert_eq!(
            board.coverings1().iter().copied().collect::<Vec<_>>(),
            vec![]
        );

        board.flip((2, 3).into());
        board.flip((4, 1).into());

        // ###
        // #..
        // #..
        assert_eq!(
            board.coverings1().iter().copied().collect::<Vec<_>>(),
            vec![-Vector::new(2, 1)]
        );

        board.flip((3, 2).into());
        board.flip((3, 3).into());

        // ###
        // ###
        // #..
        assert_eq!(
            board.coverings1().iter().copied().collect::<Vec<_>>(),
            vec![-Vector::new(3, 1), -Vector::new(2, 1)]
        );
    }

    #[test]
    fn test_coverings1_zero_origin() {
        // ..#
        // ###
        let problem = TightPoly::from_str("2 3\n..#\n###").unwrap();
        let mut board = Cover1Queries::new(problem, 5);

        assert_eq!(board.coverings1().len(), 0);

        board.flip((2, 3).into());
        board.flip((3, 1).into());
        board.flip((3, 2).into());
        board.flip((3, 3).into());

        // ..#
        // ###
        assert_eq!(
            board.coverings1().iter().copied().collect::<Vec<_>>(),
            vec![-Vector::new(2, 1)]
        );
    }
}

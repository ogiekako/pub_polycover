use super::{
    bit_poly::BitPoly,
    u256::{U128, U256},
    vector::Vector,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shallowness {
    pub depth: usize,
    pub nth: usize,
}

#[derive(Debug, Clone)]
pub struct BoardAnalyzerSymmetric {
    poly: BitPoly,

    side_bits: usize,
    side: usize,

    smallest_x: usize,
    rows: Vec<U256>,
    smallest_xy: usize,
    diags: Vec<U128>,

    row_nth: Vec<usize>,
    diag_nth: Vec<usize>,

    cols: Vec<U256>,
}

impl BoardAnalyzerSymmetric {
    pub fn new(side: usize) -> Self {
        assert!((side + 1) / 2 <= U256::BITS as usize);

        let mut rows = vec![0.into(); (side + 1) / 2];
        let mut diags = vec![0.into(); side];
        rows.push(1.into());
        diags.push(1.into());

        let cols = vec![U256::zero(); (side + 1) / 2];

        let side_bits = side.next_power_of_two().trailing_zeros() as usize;

        Self {
            poly: BitPoly::new(side, side),

            side_bits,
            side,

            smallest_x: rows.len() - 1,
            rows,
            smallest_xy: diags.len() - 1,
            diags,

            row_nth: vec![0; (1 << side_bits) * (side + 1) / 2],
            diag_nth: vec![0; (1 << side_bits) * (side + 1) / 2],

            cols,
        }
    }

    #[inline(always)]
    fn normalize(&self, v: Vector) -> (usize, usize) {
        debug_assert!(v.x >= 0 && v.y >= 0);

        let x = v.x.min(self.side as i32 - 1 - v.x);
        let y = v.y.min(self.side as i32 - 1 - v.y);

        if x > y {
            (y as usize, x as usize)
        } else {
            (x as usize, y as usize)
        }
    }

    pub fn flip(&mut self, v: Vector) {
        self.poly.flip(&v);

        let (x, y) = self.normalize(v);
        if v.x as usize != x || v.y as usize != y {
            return;
        }

        self.rows[x].flip(y - x);
        self.cols[y].flip(y - x);

        if x < self.smallest_x && !self.rows[x].is_zero() {
            self.smallest_x = x;
        } else if x == self.smallest_x && self.rows[x].is_zero() {
            self.smallest_x += 1;

            while self.rows[self.smallest_x].is_zero() {
                self.smallest_x += 1;
            }
        }

        let xy = x + y;
        let i = (y - x) / 2;
        self.diags[xy].flip(i);

        if xy < self.smallest_xy && !self.diags[xy].is_zero() {
            self.smallest_xy = xy;
        } else if xy == self.smallest_xy && self.diags[xy].is_zero() {
            self.smallest_xy += 1;

            while self.diags[self.smallest_xy].is_zero() {
                self.smallest_xy += 1;
            }
        }

        for y1 in x..(self.side + 1) / 2 {
            self.row_nth[x << self.side_bits | y1] = if y1 == x {
                0
            } else {
                self.row_nth[x << self.side_bits | y1 - 1] + self.rows[x].get(y1 - 1 - x) as usize
            }
        }

        for x1 in 0..(self.side + 1) / 2 {
            if xy < x1 * 2 {
                break;
            }
            let y1 = xy - x1;
            if y1 > (self.side + 1) / 2 {
                continue;
            }

            self.diag_nth[x1 << self.side_bits | y1] = if x1 == 0 {
                0
            } else {
                self.diag_nth[(x1 - 1) << self.side_bits | (y1 + 1)]
                    + self.diags[xy].get((y1 + 1 - (x1 - 1)) >> 1) as usize
            }
        }
    }

    #[inline(always)]
    pub fn shallowness(&self, p: Vector) -> [Shallowness; 2] {
        let (x, y) = self.normalize(p);

        [
            {
                Shallowness {
                    depth: x - self.smallest_x,
                    nth: self.row_nth[x << self.side_bits | y],
                }
            },
            {
                Shallowness {
                    depth: x + y - self.smallest_xy,
                    nth: self.diag_nth[x << self.side_bits | y],
                }
            },
        ]
    }

    // How many cells are set in front of p excluding p.
    pub fn frontness(&self, p: Vector) -> usize {
        let (x, y) = self.normalize(p);
        self.cols[y].count_ones_gt(y - x) as usize
    }

    #[inline(always)]
    pub(crate) fn get(&self, p: Vector) -> bool {
        self.poly.get_v(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shallowness() {
        let mut analyzer = BoardAnalyzerSymmetric::new(19);

        // #..###
        //  .#.#.
        //   ##..

        analyzer.flip(Vector::new(0, 0));
        analyzer.flip(Vector::new(0, 3));
        analyzer.flip(Vector::new(0, 4));
        analyzer.flip(Vector::new(0, 5));
        analyzer.flip(Vector::new(1, 2));
        analyzer.flip(Vector::new(1, 4));
        analyzer.flip(Vector::new(2, 2));
        analyzer.flip(Vector::new(2, 3));

        assert_eq!(
            analyzer.shallowness(Vector::new(0, 0)),
            [
                (Shallowness { depth: 0, nth: 0 }),
                (Shallowness { depth: 0, nth: 0 })
            ]
        );
        assert_eq!(
            analyzer.shallowness(Vector::new(0, 3)),
            [
                (Shallowness { depth: 0, nth: 1 }),
                (Shallowness { depth: 3, nth: 0 })
            ]
        );
        assert_eq!(
            analyzer.shallowness(Vector::new(1, 2)),
            [
                (Shallowness { depth: 1, nth: 0 }),
                (Shallowness { depth: 3, nth: 1 })
            ]
        );
        assert_eq!(
            analyzer.shallowness(Vector::new(2, 3)),
            [
                (Shallowness { depth: 2, nth: 1 }),
                (Shallowness { depth: 5, nth: 2 })
            ]
        );
    }

    #[test]
    fn test_frontness() {
        let mut analyzer = BoardAnalyzerSymmetric::new(19);

        // #..##
        //  .#..
        //   #..

        analyzer.flip(Vector::new(0, 0));
        analyzer.flip(Vector::new(0, 3));
        analyzer.flip(Vector::new(0, 4));
        analyzer.flip(Vector::new(1, 2));
        analyzer.flip(Vector::new(2, 2));

        assert_eq!(analyzer.frontness(Vector::new(0, 0)), 0);
        assert_eq!(analyzer.frontness(Vector::new(0, 2)), 0);
        assert_eq!(analyzer.frontness(Vector::new(2, 2)), 1);
        assert_eq!(analyzer.frontness(Vector::new(2, 3)), 1);
    }
}

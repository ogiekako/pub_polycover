use std::rc::Rc;

use super::{
    bit_poly::BitPoly, sparse_t2_query::SparseT2Query, t2_counter::T2Counter, t2_masks::T2Masks,
    tight_poly::TightPoly, vector::Vector,
};

#[derive(Clone)]
pub struct Cover2Queries {
    problem: TightPoly,
    problem_cells: Rc<Vec<Vector>>,

    height: usize,
    width: usize,

    a: Vec<BitPoly>,
    b: Vec<BitPoly>,

    // i >= j
    // dj - di \in stq[i][j] <=> (di + ai) \cap (dj + bj) == \emptyset
    stq: Vec<Vec<SparseT2Query>>,

    // d -> |ai \cap (d + bj)|
    counter: Vec<Vec<T2Counter>>,

    inner: Inner,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Inner {
    full_mask: usize,

    // [-max_mask_corner, -min_mask_corner] -> mask
    ma_inv: Vec<T2Masks>,
    // [min_mask_corner, max_mask_corner] -> mask
    mb: Vec<T2Masks>,

    coverings2: Vec<Vec<Vec<(Vector, Vector)>>>,
}

impl Cover2Queries {
    pub fn new(problem: TightPoly, height: usize, width: usize, n: usize) -> Self {
        let a = vec![BitPoly::new(height, width); n];
        let b = a.clone();

        let min_mask_corner: Vector = (-(height as i32) + 1, -(width as i32) + 1).into();
        let mut max_mask_corner: Vector =
            (problem.height() as i32 - 1, problem.width() as i32 - 1).into();

        while (max_mask_corner.x - min_mask_corner.x + 1) % 4 != 0 {
            max_mask_corner.x += 1;
        }
        while (max_mask_corner.y - min_mask_corner.y + 1) % 4 != 0 {
            max_mask_corner.y += 1;
        }

        let query_height = (max_mask_corner.x - min_mask_corner.x + 1) as usize;
        let query_width = (max_mask_corner.y - min_mask_corner.y + 1) as usize;

        let min = min_mask_corner - max_mask_corner;
        let max = max_mask_corner - min_mask_corner;

        let mut counter = vec![vec![]; n];
        let mut stq = vec![vec![]; n];
        let mut coverings2 = vec![vec![]; n];

        for i in 0..n {
            for _ in 0..i + 1 {
                coverings2[i].push(vec![]);
                counter[i].push(T2Counter::new(min, max));
                let mut stq0 = SparseT2Query::new(min, max, query_height, query_width);
                for x in min.x..=max.x {
                    for y in min.y..=max.y {
                        stq0.flip(&Vector::new(x, y));
                    }
                }
                stq[i].push(stq0);
            }
        }

        let problem_cells: Vec<_> = problem.cells();
        let full_mask = usize::MAX >> (usize::BITS - problem_cells.len() as u32);

        let ma_inv = vec![T2Masks::new(-max_mask_corner, -min_mask_corner, full_mask); n];
        let mb = vec![T2Masks::new(min_mask_corner, max_mask_corner, full_mask); n];

        Self {
            problem,
            problem_cells: Rc::new(problem_cells),

            height,
            width,

            a,
            b,
            stq,
            counter,

            inner: Inner {
                full_mask,
                ma_inv,
                mb,
                coverings2,
            },
        }
    }

    pub fn coverings1(&self, i: usize) -> impl Iterator<Item = Vector> + '_ {
        let min = self.inner.mb[i].min();
        self.inner.mb[i]
            .rect(self.inner.full_mask)
            .iter()
            .map(move |p| p + min)
    }

    pub fn coverings2(&self, i: usize, j: usize) -> &[(Vector, Vector)] {
        &self.inner.coverings2[i][j]
    }

    pub fn flip(&mut self, i: usize, p: Vector) {
        debug_assert!((0..self.height as i32).contains(&p.x));
        debug_assert!((0..self.width as i32).contains(&p.y));

        let add_a = self.a[i].flip(&p);

        self.update(add_a, true, i, &p);

        let add_b = self.b[i].flip(&p);

        self.update(add_b, false, i, &p);

        debug_assert_eq!(add_a, add_b);
    }

    fn update(&mut self, add: bool, a: bool, i: usize, p: &Vector) {
        for j in 0..self.a.len() {
            let (i, j) = if a { (i, j) } else { (j, i) };
            if i < j {
                continue;
            }
            self.update_counters(add, i, j, a, p);
        }
        self.update_mask(i, a, p);
    }

    fn update_counters(&mut self, add: bool, i: usize, j: usize, a: bool, p: &Vector) {
        let iter = if a {
            self.b[j].iter()
        } else {
            self.a[i].iter()
        };

        let counter = &mut self.counter[i][j];
        let stq = &mut self.stq[i][j];
        let inner = &mut self.inner;

        let min = *counter.min();

        for x in iter {
            let d = if a { p - &x } else { &x - p };
            let di = Vector::new(
                (d.x as isize - min.x) as usize,
                (d.y as isize - min.y) as usize,
            );

            let c = counter.get_raw_mut(&di);

            if add {
                *c += 1;
                if *c != 1 {
                    continue;
                }
            } else {
                *c -= 1;
                if *c != 0 {
                    continue;
                }
            }

            inner.update_coverings2_for_feasible_set(!add, i, j, &d);
            stq.flip(&d);
        }
    }

    fn update_mask(&mut self, i: usize, a: bool, p: &Vector) {
        for (k, q) in self.problem_cells.clone().iter().enumerate() {
            self.update_mask_bit(i, a, &(q - p), 1 << k);
        }
    }

    fn update_mask_bit(&mut self, i: usize, a: bool, x: &Vector, k: usize) {
        let flipped = if a {
            self.inner.ma_inv[i].flip(&(-x), k)
        } else {
            self.inner.mb[i].flip(x, k)
        };

        self.update_coverings2_for_mask(false, i, a, x, flipped ^ k);
        self.update_coverings2_for_mask(true, i, a, x, flipped);
    }

    fn update_coverings2_for_mask(
        &mut self,
        add: bool,
        i: usize,
        a: bool,
        x: &Vector,
        mask: usize,
    ) {
        let remaining_mask = self.inner.full_mask & !mask;
        if mask == 0 || remaining_mask == 0 {
            return;
        }

        if !add {
            if a {
                for j in 0..=i {
                    self.inner.coverings2[i][j].retain(|(da, _)| da != x);
                }
            } else {
                for j in i..self.a.len() {
                    self.inner.coverings2[j][i].retain(|(_, db)| db != x);
                }
            }
            return;
        }

        let edge = i + (mask & 1);
        if a {
            let neg_x = -x;

            let c2 = &mut self.inner.coverings2[i];
            let stq = &mut self.stq[i];

            for j in 0..edge {
                let coverings2 = &mut c2[j];
                for db_minus_da in stq[j].query(
                    neg_x + self.inner.mb[j].min(),
                    self.inner.mb[j].rect(remaining_mask),
                ) {
                    coverings2.push((*x, &db_minus_da + x));
                }
            }
        } else {
            for j in edge..self.a.len() {
                let coverings2 = &mut self.inner.coverings2[j][i];
                for db_minus_da in self.stq[j][i].query(
                    x + self.inner.ma_inv[j].min(),
                    self.inner.ma_inv[j].rect(remaining_mask),
                ) {
                    coverings2.push((x - &db_minus_da, *x));
                }
            }
        }
    }

    pub fn overlap_unchecked(
        &self,
        i: usize,
        j: usize,
        da: &Vector<isize>,
        db: &Vector<isize>,
    ) -> bool {
        self.counter[i][j].get_unchecked(&(db - da)) != 0
    }

    pub fn overlap(&self, i: usize, j: usize, da: &Vector<isize>, db: &Vector<isize>) -> bool {
        self.counter[i][j].get(&(db - da)).unwrap_or(0) != 0
    }

    pub fn overlap_count(
        &self,
        i: usize,
        j: usize,
        da: &Vector<isize>,
        db: &Vector<isize>,
    ) -> usize {
        self.counter[i][j].get(&(db - da)).unwrap_or(0)
    }

    pub(crate) fn all_placements(
        &self,
        i: usize,
        max_count: u32,
    ) -> impl Iterator<Item = (usize, Vector)> + '_ {
        let mb = &self.inner.mb[i];
        let min = mb.min();

        let h = mb.height();
        let w = mb.width();

        (0..h).flat_map(move |x| {
            (0..w).filter_map(move |y| {
                let mask = mb.get_raw(x, y);
                if mask == 0 || mask.count_ones() > max_count {
                    None
                } else {
                    Some((mask, Vector::new(x as i32 + min.x, y as i32 + min.y)))
                }
            })
        })
    }
}

impl Inner {
    fn update_coverings2_for_feasible_set(
        &mut self,
        is_feasible: bool,
        i: usize,
        j: usize,
        db_minus_da: &Vector,
    ) {
        if !is_feasible {
            self.coverings2[i][j].retain(|(da, db)| &(db - da) != db_minus_da);
            return;
        }

        let ma_inv = &self.ma_inv[i];
        let mb = &self.mb[j];
        let a_must_cover0 = i == j;
        let coverings2 = &mut self.coverings2[i][j];

        let a_min_x = ma_inv.min().x as isize;
        let a_min_y = ma_inv.min().y as isize;

        let b_min_x = mb.min().x as isize;
        let b_min_y = mb.min().y as isize;

        let (bx2, by2, bx1, by1) = {
            let dx = db_minus_da.x as isize;
            let dy = db_minus_da.y as isize;

            (
                (dx.min(0) + (mb.max().x as isize) - b_min_x) as usize + 1,
                (dy.min(0) + (mb.max().y as isize) - b_min_y) as usize + 1,
                dx.max(0) as usize,
                dy.max(0) as usize,
            )
        };

        let ab_x = (db_minus_da.x as isize - a_min_x - b_min_x) as usize;
        let ab_y = (db_minus_da.y as isize - a_min_y - b_min_y) as usize;

        for bx in bx1..bx2 {
            for by in by1..by2 {
                let other_mask = mb.get_raw(bx, by);
                let mask = self.full_mask & !other_mask;

                let (ax, ay) = (ab_x - bx, ab_y - by);

                if other_mask != 0 && mask != 0 && ma_inv.get_raw(ax, ay) == mask {
                    if a_must_cover0 && mask & 1 == 0 {
                        continue;
                    }
                    let a = -Vector::new(
                        (ax as isize + a_min_x) as i32,
                        (ay as isize + a_min_y) as i32,
                    );
                    let b = Vector::new(
                        (bx as isize + b_min_x) as i32,
                        (by as isize + b_min_y) as i32,
                    );
                    coverings2.push((a, b));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::data::{cover2_queries::Cover2Queries, tight_poly::TightPoly, vector::Vector};

    #[test]
    fn test_flip_small() {
        let problem = TightPoly::from_str(
            r#"2 2
##
#.
"#,
        )
        .unwrap();

        let mut c2q = Cover2Queries::new(problem, 3, 3, 2);

        c2q.flip(0, Vector::new(0, 0));
        c2q.flip(1, Vector::new(0, 0));

        assert_eq!(c2q.coverings2(1, 0).len(), 0);

        // a: ##
        // b: #
        c2q.flip(0, Vector::new(0, 1));

        assert_eq!(c2q.coverings2(1, 0), vec![((1, 0).into(), (0, 0).into())]);

        c2q.flip(0, Vector::new(0, 0));
        assert_eq!(c2q.coverings2(1, 0).len(), 0);

        c2q.flip(0, Vector::new(0, 2));
        // a: .##
        // b: #
        assert_eq!(c2q.coverings2(1, 0), vec![((1, 0).into(), (0, -1).into())]);

        c2q.flip(1, Vector::new(1, 0));
        let mut c2 = c2q.coverings2(1, 0).to_vec();
        c2.sort();
        assert_eq!(
            c2,
            vec![
                ((0, 0).into(), (0, 0).into()),
                ((1, 0).into(), (0, -1).into()),
            ]
        );

        c2q.flip(0, Vector::new(0, 1));
        // a: ..#
        // b: #
        //    #
        assert_eq!(c2q.coverings2(1, 0), vec![((0, 0).into(), (0, -1).into())]);

        c2q.flip(1, Vector::new(0, 0));
        c2q.flip(1, Vector::new(1, 0));
        c2q.flip(1, Vector::new(1, 1));
        c2q.flip(1, Vector::new(2, 1));
        // a: ..#
        // b: ..
        //    .#
        //    .#
        assert_eq!(
            c2q.coverings2(1, 0),
            vec![((-1, -1).into(), (0, -1).into())]
        );
    }

    #[test]
    fn test_flip() {
        for (problem_str, a, b, want) in [
            (
                r#"3 2
#.
##
#.
                "#,
                r#"3 2
#.
##
.#
                "#,
                r#"1 2
##
                "#,
                vec![((0, 0).into(), (2, -1).into())],
            ),
            (
                r#"3 3
.#.
###
.#.
                "#,
                r#"3 4
.#.#
####
#.#.
                "#,
                r#"2 3
###
.#.
                "#,
                //
                // .A.A | ..A.A | .oBB | ...... | .BBB
                // AAAA | .AAAA | .ABA | BoBA.A | .oBA
                // AoA. | oA.A. | AAAA | .BAAAA | AAAA
                // .BBB | BBB   | A.A. | ..A.A. | A.A.
                // ..B. | .B.   | .... | ...... | ....
                vec![
                    ((-2, -1).into(), (1, 0).into()),
                    ((-2, 1).into(), (1, 0).into()),
                    ((0, -1).into(), (-1, 0).into()),
                    ((0, 1).into(), (0, -1).into()),
                    ((1, -1).into(), (0, 0).into()),
                ],
            ),
            (
                r#"3 3
.#.
###
.#.
                "#,
                r#"11 11
.####.#....
.#..###..##
.....#....#
#...##....#
##...###.##
.###.#.###.
##.###...##
#....##...#
#....#.....
##..###..#.
...##.####.     
                "#,
                r#"11 11
.####.##...
.#..###..##
.....#....#
#...##....#
##...###.##
.###.#.###.
##.###...##
#....##...#
#....#.....
##..###..#.
....#.####.                
                "#,
                //   ..oB..
                //   .obBB.
                //   AaBABB
                //   .AAA..
                //   ..A...
                vec![
                    ((0, -4).into(), (-10, -3).into()),
                    ((1, -5).into(), (-9, -4).into()),
                    ((1, -4).into(), (-9, -3).into()),
                    ((2, -5).into(), (-8, -4).into()),
                ],
            ),
        ] {
            let problem = TightPoly::from_str(problem_str).unwrap();
            let a = TightPoly::from_str(a).unwrap();
            let b = TightPoly::from_str(b).unwrap();

            let mut c2q = Cover2Queries::new(problem, 11, 11, 2);

            for p in a.cells() {
                c2q.flip(0, p);
            }
            for p in b.cells() {
                c2q.flip(1, p);
            }

            let mut c2 = c2q.coverings2(1, 0).to_vec();
            c2.sort();
            let mut want = want.into_iter().map(|(a, b)| (b, a)).collect::<Vec<_>>();
            want.sort();
            assert_eq!(c2, want, "{:?}", problem_str);
        }
    }

    #[test]
    fn test_fast_update() {
        let problem = TightPoly::from_str(
            r#"3 6
###.##
######
##.###
"#,
        )
        .unwrap();

        let mut c2q = Cover2Queries::new(problem, 3, 3, 2);
        c2q.flip(0, Vector::new(0, 0));
        c2q.flip(0, Vector::new(0, 1));
        c2q.flip(0, Vector::new(0, 2));
        c2q.flip(0, Vector::new(1, 0));
        c2q.flip(0, Vector::new(1, 2));
        c2q.flip(0, Vector::new(2, 0));
        c2q.flip(0, Vector::new(2, 1));
        c2q.flip(0, Vector::new(2, 2));

        c2q.flip(1, Vector::new(0, 0));
        c2q.flip(1, Vector::new(0, 1));
        c2q.flip(1, Vector::new(0, 2));
        c2q.flip(1, Vector::new(1, 0));
        c2q.flip(1, Vector::new(1, 1));
        c2q.flip(1, Vector::new(1, 2));
        c2q.flip(1, Vector::new(2, 0));
        c2q.flip(1, Vector::new(2, 1));

        assert!(c2q.coverings2(1, 0).is_empty());

        c2q.flip(0, Vector::new(1, 1));
        assert_eq!(c2q.coverings2(1, 0), vec![((0, 0).into(), (0, 3).into())]);

        c2q.flip(0, Vector::new(1, 1));
        assert_eq!(c2q.coverings2(1, 0), vec![]);

        c2q.flip(0, Vector::new(1, 1));
        assert_eq!(c2q.coverings2(1, 0), vec![((0, 0).into(), (0, 3).into())]);

        c2q.flip(1, Vector::new(1, 1));
        assert_eq!(c2q.coverings2(1, 0), vec![]);

        c2q.flip(1, Vector::new(1, 1));
        assert_eq!(c2q.coverings2(1, 0), vec![((0, 0).into(), (0, 3).into())]);

        c2q.flip(1, Vector::new(2, 2));
        assert_eq!(
            c2q.coverings2(1, 0),
            vec![
                ((0, 0).into(), (0, 3).into()),
                ((0, 3).into(), (0, 0).into()),
            ]
        );
        c2q.flip(0, Vector::new(2, 2));
        assert_eq!(c2q.coverings2(1, 0), vec![((0, 3).into(), (0, 0).into())]);

        c2q.flip(1, Vector::new(0, 0));
        assert_eq!(c2q.coverings2(1, 0), vec![((0, 3).into(), (0, 0).into())]);
    }
}

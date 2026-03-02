use std::{collections::BTreeMap, sync::Arc};

use anyhow::{ensure, Result};
use petgraph::unionfind::UnionFind;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use super::{d4::D4, tight_poly::TightPoly, vector::Vector};

#[derive(Clone, PartialEq, PartialOrd, Ord, Eq, Debug)]
struct Surroundings {
    // (-1,-1), ..., (-1,n), (0,-1), (0,n), ..., (n-1,n), (n,-1), ..., (n,n)
    components: Vec<Option<u8>>,
}

impl Surroundings {
    fn try_from_mask(mask: usize, side: usize) -> Result<Self> {
        let mut uf = UnionFind::new(side * side + 1);
        let outside = side * side;

        let forbidden_block = [
            // ##      #.      .#
            // ##      .#      #.
            0b1111, 0b1001, 0b0110,
        ];

        for x in 0..side {
            for y in 0..side {
                if x > 0 && y > 0 {
                    let blk = mask >> ((x - 1) * side + y - 1) & 3
                        | (mask >> (x * side + y - 1) & 3) << 2;
                    ensure!(
                        !forbidden_block.contains(&blk),
                        "Invalid 2x2 pattern exists"
                    );
                }

                let i = x * side + y;

                let v = Vector::new(x as i32, y as i32);

                for u in v.neighbors4() {
                    if !(0..side as i32).contains(&u.x) || !(0..side as i32).contains(&u.y) {
                        if mask >> i & 1 == 0 {
                            uf.union(i, outside);
                        }
                        continue;
                    }
                    let j = (u.x * side as i32 + u.y) as usize;
                    if mask >> i & 1 != mask >> j & 1 {
                        continue;
                    }
                    uf.union(i, j);
                }
            }
        }
        let mut mapping = BTreeMap::new();

        let mut components = vec![];
        for x in 0..side {
            for y in 0..side {
                let i = x * side + y;
                ensure!(mask >> i & 1 == 1 || uf.equiv(i, outside), "Has a hole");

                if (1..side - 1).contains(&x) && (1..side - 1).contains(&y) {
                    continue;
                }
                if mask >> i & 1 == 0 {
                    components.push(None);
                    continue;
                }
                let r = uf.find(i);

                let l = mapping.len();
                components.push(Some((*mapping.entry(r).or_insert(l)) as u8));
            }
        }
        for x in 1..side - 1 {
            for y in 1..side - 1 {
                let i = x * side + y;
                if mask >> i & 1 == 0 {
                    continue;
                }

                ensure!(
                    mapping.contains_key(&uf.find(i)),
                    "Has isolated component {:b}",
                    mask
                );
            }
        }
        Ok(Self { components })
    }
}

pub struct Connections {
    side: usize,
    // surrounding mask -> all interior masks
    mapping: Vec<Arc<Vec<usize>>>,
}

impl Connections {
    pub fn new(interior_side: usize) -> Self {
        Self::new_inner(interior_side, None)
    }

    pub fn new_with_problem(interior_side: usize, problem: &TightPoly) -> Self {
        Self::new_inner(interior_side, Some(problem))
    }

    fn new_inner(interior_side: usize, problem: Option<&TightPoly>) -> Self {
        let forbidden_block = [
            // ##      #.      .#
            // ##      .#      #.
            0b1111, 0b1001, 0b0110,
        ];

        let side = interior_side + 2;

        let mut forbidden_masks = vec![];
        if let Some(problem) = problem {
            for d in D4::all() {
                let mut prob = problem.clone();
                if d.get_flip() {
                    prob = prob.flipped();
                }
                for _ in 0..d.get_rot() {
                    prob = prob.roted90();
                }
                let (h, w) = (prob.height(), prob.width());
                if side + 1 < h || side + 1 < w {
                    continue;
                }
                for x in 0..side + 1 - h {
                    for y in 0..side + 1 - w {
                        let mut mask = 0;
                        for i in 0..h {
                            for j in 0..w {
                                let v = Vector::new(i as i32, j as i32);
                                if prob.get_v(v) {
                                    mask |= 1 << ((x + i) * side + y + j);
                                }
                            }
                        }
                        forbidden_masks.push(mask);
                    }
                }
            }
            forbidden_masks.sort();
            forbidden_masks.dedup();
        }

        let mut valid_nexts = vec![vec![]; 1 << side];
        for i in 0..1 << side {
            for j in 1..1 << side {
                let mut ok = true;
                for k in 0..side - 1 {
                    let mut m = 0;
                    m |= i >> k & 3;
                    m |= (j >> k & 3) << 2;
                    if forbidden_block.contains(&m) {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    valid_nexts[i].push(j);
                }
            }
        }

        let mut dp = vec![0];

        for x in 0..side {
            for j in 0..dp.len() {
                let prev_mask = dp[j];
                let prev_line = if x == 0 {
                    0
                } else {
                    prev_mask >> (x - 1) * side
                };

                for add in valid_nexts[prev_line].iter().copied() {
                    let mask = prev_mask | (add << x * side);
                    if forbidden_masks.iter().all(|&x| mask & x != x) {
                        dp.push(mask);
                    }
                }
            }
        }

        let mut map = BTreeMap::<Surroundings, Vec<usize>>::new();

        let sms: Vec<_> = dp
            .into_par_iter()
            .filter_map(|mask| {
                if let Ok(s) = Surroundings::try_from_mask(mask, side) {
                    let mut m = 0;
                    for i in 0..interior_side {
                        let from = (i + 1) * side + 1;
                        let to = from + interior_side;
                        m |= (mask & (1 << to) - (1 << from)) >> from << i * interior_side;
                    }
                    Some((s, mask, m))
                } else {
                    None
                }
            })
            .collect();

        let mut mapping = vec![];
        for _ in 0..1 << side * side {
            mapping.push(vec![].into());
        }
        for (s, _, m) in sms.iter() {
            map.entry(s.clone()).or_insert(vec![]).push(*m);
        }
        let map = map
            .into_iter()
            .map(|(k, v)| (k, Arc::new(v)))
            .collect::<BTreeMap<_, _>>();

        for (s, mask, _) in sms.iter() {
            mapping[*mask] = map.get(s).unwrap().clone();
        }

        Self { side, mapping }
    }

    pub(crate) fn candidates(&self, mask_2d: usize) -> Arc<Vec<usize>> {
        self.mapping[mask_2d].clone()
    }
}

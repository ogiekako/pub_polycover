use std::collections::BTreeMap;

use crate::{
    data::{board::Board, d4::D4, e2::E2, outline::Outline, vector::Vector},
    solver::parameters::Parameters,
};

use super::Penalizer;

const COV1_COEFF: f64 = 1e15;
const POWER: f64 = 1.0;

pub struct StandardPenalizer {
    params: Parameters,
}

impl Penalizer for StandardPenalizer {
    fn init(&mut self, params: Parameters) {
        self.params = params;
    }

    fn penalty_per_covering(
        self: &Self,
        board: &Board,
        include_other_coverings: bool,
        sim: &[D4],
    ) -> Vec<(f64, Vec<E2>)> {
        self.penalty_per_covering_inner(board, include_other_coverings, sim)
    }

    fn penalty_per_covering_sym(&self, board: &Board, ppc: &mut Vec<(f64, Vector, Vector)>) {
        // TODO: update
        self.penalty_per_covering_inner(board, false, &D4::all().collect::<Vec<_>>())
            .into_iter()
            .for_each(|(p, ms)| ppc.push((p, ms[0].d, ms[1].d)))
    }

    fn clone(self: &Self) -> Box<dyn Penalizer> {
        Box::new(Self {
            params: self.params.clone(),
        })
    }
}

impl StandardPenalizer {
    pub fn new() -> Self {
        Self {
            params: Parameters::default(),
        }
    }

    fn penalty_per_covering_inner(
        &self,
        board: &Board,
        _include_other_coverings: bool,
        sim: &[D4],
    ) -> Vec<(f64, Vec<E2>)> {
        let mut ps = vec![];

        self.penalty_per_cover2(board, sim, &mut ps);

        if ps.is_empty() {
            return ps;
        }

        let max_i = (0..ps.len())
            .max_by(|x, y| ps[*x].0.partial_cmp(&ps[*y].0).unwrap())
            .unwrap();
        if max_i > 0 {
            ps.swap(0, max_i);
        }

        // Add penalty if bounding box is too small.
        let bb = board.bounding_box();
        let ratio = bb.area() as f64 / (board.height() * board.width()) as f64;
        let mult = 8. - 7. * ratio;
        ps.iter_mut().for_each(|x| {
            x.0 *= mult;
        });

        if !ps.is_empty() {
            let mul2 = (ps[0].0 / 100.0).max(1.0).min(5.0);
            ps.iter_mut().for_each(|x| {
                x.0 *= mul2;
            });
        }

        ps
    }

    #[inline(never)]
    fn penalty_per_cover2(&self, board: &Board, sim: &[D4], ps: &mut Vec<(f64, Vec<E2>)>) {
        let center = Vector::new(board.height() as i32 / 2, board.width() as i32 / 2);

        let cov2 = board.coverings2();

        let mut same_position: BTreeMap<E2, usize> = Default::default();

        for (m1, m2) in cov2.iter() {
            if self.params.pen_depenalize_same_position {
                {
                    let m = m1 * &m2.inverse();
                    *same_position.entry(m).or_insert(0) += 1;
                }
            }

            let unplug = board.unplug_len_per_direction8(m1, m2);
            let sum = unplug
                .iter()
                .zip(Vector::directions8())
                .map(|(u, d)| {
                    let mut r = 1. / (*u as f64 + 1.).powf(POWER);
                    if d.x != 0 && d.y != 0 {
                        r *= 1.1;
                    } else {
                        r /= 1.1;
                    }
                    r
                })
                .sum();

            let mut coeff = 1f64;

            self.penalize_nearby_cell_contributions(board, m1, m2, &center, &mut coeff, sim);

            let cr = board.common_rect(m1, m2);
            if cr.is_empty() {
                coeff += 1.0;
            } else if cr.height().min(cr.width()) == 1 {
                coeff += 0.2;
            }

            {
                let mut friction = 0;
                for nd in m1.d.neighbors4() {
                    let mut m = *m1;
                    m.d = nd;
                    friction += board.overlap_count(&m, m2);
                }
                coeff += 1. / (friction as f64).max(1.0).log2();
            }

            let p = 2.5f64.powf(sum) * coeff.min(10.0);
            ps.push((p, vec![*m1, *m2]));
        }
        if self.params.pen_depenalize_same_position {
            ps.iter_mut().for_each(|(p, ms)| {
                let m = &ms[0] * &ms[1].inverse();
                let dup = *same_position.get(&m).unwrap() as f64;
                *p /= dup.sqrt();
            });
        }
    }

    #[inline(never)]
    fn penalize_nearby_cell_contributions(
        &self,
        board: &Board,
        m1: &E2,
        m2: &E2,
        center: &Vector,
        coeff: &mut f64,
        sim: &[D4],
    ) {
        let l = Outline::new(board.height(), board.width());

        let mut mask1 = 0;
        for (i, p) in board.problem_cells().iter().enumerate() {
            if board.get(&m1.inverse() * p) {
                mask1 |= 1 << i;
            }
        }

        let m1i = m1.inverse();
        let m2i = m2.inverse();

        for cover1 in [true, false] {
            let mut use_d: Option<D4> = None;
            for (i, p) in board.problem_cells().iter().enumerate() {
                if (mask1 >> i & 1 == 1) != cover1 {
                    continue;
                }
                let (q, om) = if cover1 {
                    (&m1i * p, m2)
                } else {
                    (&m2i * p, m1)
                };

                if board.tree().is_leaf(q) {
                    *coeff += 0.1;
                }

                let dist = (q.x - center.x).abs().max((q.x - center.x).abs()) as f64;
                if dist < 3.0 {
                    *coeff += 8.0 * (3.0 - dist) / 3.0;
                }

                let mut min_dist = usize::MAX;

                let c = l.cell(q.x as usize, q.y as usize);

                for d in sim {
                    if let Some(use_d) = use_d.as_ref() {
                        if use_d != d {
                            continue;
                        }
                    }

                    let (x, y) = c.transform(*d);
                    let r = om * &Vector::new(x as i32, y as i32);
                    if r.x.abs() + r.y.abs() >= 10 {
                        continue;
                    }
                    for (j, pc) in board.problem_cells().iter().enumerate() {
                        if (mask1 >> j & 1 == 1) == cover1 {
                            continue;
                        }
                        let dist = ((r.x - pc.x).abs() + (r.y - pc.y).abs()) as usize;
                        if dist < min_dist {
                            min_dist = dist;
                            use_d = Some(*d);
                        }
                    }
                    if min_dist == 0 {
                        break;
                    }
                }

                *coeff += match min_dist {
                    0 | 1 | 2 => 0.5 / (1 << min_dist) as f64,
                    _ => 0.,
                };
            }
        }
    }
}

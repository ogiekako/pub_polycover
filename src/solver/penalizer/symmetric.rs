use std::collections::BTreeMap;

use crate::{
    data::{board::Board, d4::D4, vector::Vector},
    solver::parameters::Parameters,
};

use super::Penalizer;

const COV1_COEFF: f64 = 1e15;

pub struct SymmetricPenalizer {
    params: Parameters,
}

impl Penalizer for SymmetricPenalizer {
    fn init(&mut self, params: Parameters) {
        self.params = params;
    }

    fn penalty_per_covering(
        self: &Self,
        _board: &Board,
        _include_other_coverings: bool,
        _sim: &[D4],
    ) -> Vec<(f64, Vec<crate::data::e2::E2>)> {
        unimplemented!()
    }

    fn penalty_per_covering_sym(&self, board: &Board, ppc: &mut Vec<(f64, Vector, Vector)>) {
        self.penalty_per_covering_inner(board, ppc)
    }

    fn clone(self: &Self) -> Box<dyn Penalizer> {
        Box::new(Self {
            params: self.params.clone(),
        })
    }
}

impl SymmetricPenalizer {
    pub fn new() -> Self {
        Self {
            params: Parameters::default(),
        }
    }

    fn penalty_per_covering_inner(&self, board: &Board, ppc: &mut Vec<(f64, Vector, Vector)>) {
        let orig_len = ppc.len();
        self.penalty_per_cover2(board, ppc);

        let ppc = &mut ppc[orig_len..];

        if ppc.is_empty() {
            return;
        }

        let max_i = (0..ppc.len())
            .max_by(|x, y| ppc[*x].0.partial_cmp(&ppc[*y].0).unwrap())
            .unwrap();
        ppc.swap(0, max_i);

        let mul2 = (ppc[0].0 / 20.0).max(1.0).min(5.0);
        ppc.iter_mut().for_each(|x| {
            x.0 *= mul2;
        });
    }

    #[inline(never)]
    fn penalty_per_cover2(&self, board: &Board, ppc: &mut Vec<(f64, Vector, Vector)>) {
        let orig_len = ppc.len();
        for (m1, m2) in board.coverings2_symmetric() {
            ppc.push((0., *m1, *m2));
        }

        let ppc = &mut ppc[orig_len..];

        let mut same_position: BTreeMap<Vector, usize> = Default::default();

        for (pen, m1, m2) in ppc.iter_mut() {
            if self.params.pen_depenalize_same_position {
                {
                    let m = *m1 - *m2;
                    *same_position.entry(m).or_insert(0) += 1;
                }
            }

            // let unplug = board.unplug_len_per_direction8_symmetric(m1, m2);
            // let sum = unplug
            //     .zip(Vector::directions8())
            //     .map(|(u, d)| {
            //         let mut r = self.params.pen_unplug_coeff
            //             / (u as f64 + 1.).powf(self.params.pen_unplug_power);
            //         if d.x != 0 && d.y != 0 {
            //             r *= self.params.pen_unplug_diag_mult;
            //         } else {
            //             r /= self.params.pen_unplug_diag_mult;
            //         }
            //         r
            //     })
            //     .sum();

            let mut coeff = 1f64;

            let cr = board.common_rect_symmetric(m1, m2);
            if cr.is_empty() {
                coeff += self.params.pen_w0_common_rect;
            } else if cr.height().min(cr.width()) == 1 {
                coeff += self.params.pen_w1_common_rect;
            } else if cr.height().min(cr.width()) == 2 {
                coeff += self.params.pen_w2_common_rect;
            }

            let mut friction = 0;
            for m1n in m1.neighbors4() {
                let f = board.overlap_count_symmetric(&m1n, m2);
                friction += f;
                if f <= 0 {
                    friction -= 2;
                }
            }
            let friction_exp = self.params.pen_unplug_coeff / (friction as f64).max(1.00).sqrt();

            *pen = self.params.pen_base.powf(friction_exp) * coeff.min(10.0);
        }

        if self.params.pen_depenalize_same_position {
            ppc.iter_mut().for_each(|(p, m1, m2)| {
                let m = *m1 - *m2;
                let dup = *same_position.get(&m).unwrap() as f64;
                *p /= dup.sqrt();
            });
        }
    }
}

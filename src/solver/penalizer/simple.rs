use crate::{
    data::{
        board::Board, board_analyzer_symmetric::BoardAnalyzerSymmetric, d4::D4, e2::E2, u256::U256,
        vector::Vector,
    },
    solver::parameters::Parameters,
};

use super::Penalizer;

pub struct SimplePenalizer {
    params: Parameters,
    // pen_shallowness_depth_powers: Vec<f64>,
    // pen_shallowness_base_powers: Vec<f64>,
    pen_friction_base_powers: Vec<f64>,

    pen_shallowness_weights: Vec<f64>,
    pen_shallowness_diag_weights: Vec<f64>,
}

const D: usize = 100;

impl SimplePenalizer {
    pub fn new() -> Self {
        let params = Parameters::default();
        let mut res = Self {
            params: params.clone(),
            pen_friction_base_powers: vec![0.; 1000],
            pen_shallowness_weights: vec![0.; (U256::BITS * U256::BITS) as usize],
            pen_shallowness_diag_weights: vec![0.; (U256::BITS * U256::BITS) as usize],
        };
        res.init(params);
        res
    }

    fn penalize_small_friction(
        self: &Self,
        m1: &Vector,
        m2: &Vector,
        c2q: &crate::data::cover2_queries_symmetric::Cover2QueriesSymmetric,
        pen: &mut f64,
    ) {
        let d = m1 - m2;

        let mut friction = 0;
        for d in d.neighbors4() {
            friction += c2q.overlap_count(d).unwrap_or(0);
        }

        *pen += if friction < self.pen_friction_base_powers.len() {
            self.pen_friction_base_powers[friction]
        } else {
            self.params.pen_friction_base.powi(friction as i32)
        } * self.params.pen_friction_weight;
    }

    fn penalize_shallow_cover(
        self: &Self,
        board: &Board,
        m1: &Vector,
        m2: &Vector,
        ba: &BoardAnalyzerSymmetric,
        pen: &mut f64,
    ) {
        let mut sp = 0.0;

        let pcs = board.problem_cells();

        for p in pcs.iter().rev() {
            let mut pos = p - m1;
            if !ba.get(pos) {
                pos = p - m2;
            }

            let [s0, s1] = ba.shallowness(pos);

            sp += self.pen_shallowness_weights[s0.depth * U256::BITS as usize | s0.nth]
                + self.pen_shallowness_diag_weights[s1.depth * U256::BITS as usize | s1.nth];
        }

        *pen += sp / (pcs.len() as f64 * (1. + self.params.pen_shallowness_diag_weight));
    }

    fn shallowness_penalty(&self, depth: usize, nth: usize) -> [f64; 2] {
        let power = (depth as f64).powf(self.params.pen_shallowness_depth_power)
            + nth as f64 * self.params.pen_nth_weight;

        let w0 = (self.params.pen_shallowness_base).powf(power);
        let w1 = w0 * self.params.pen_shallowness_diag_weight;

        [w0, w1]
    }
}

impl Penalizer for SimplePenalizer {
    fn init(&mut self, params: Parameters) {
        self.params = params;
        // for i in 0..self.pen_shallowness_depth_powers.len() {
        //     self.pen_shallowness_depth_powers[i] =
        //         (i as f64).powf(self.params.pen_shallowness_depth_power);
        // }
        // for i in 0..self.pen_shallowness_base_powers.len() {
        //     self.pen_shallowness_base_powers[i] =
        //         self.params.pen_shallowness_base.powf(i as f64 / D as f64);
        // }
        for i in 0..self.pen_friction_base_powers.len() {
            self.pen_friction_base_powers[i] = self.params.pen_friction_base.powi(i as i32);
        }

        for depth in 0..U256::BITS as usize {
            for nth in 0..U256::BITS as usize {
                let [w0, w1] = self.shallowness_penalty(depth, nth);

                self.pen_shallowness_weights[depth * U256::BITS as usize | nth] = w0;
                self.pen_shallowness_diag_weights[depth * U256::BITS as usize | nth] = w1;
            }
        }
    }

    fn penalty_per_covering(
        self: &Self,
        _board: &Board,
        _include_other_coverings: bool,
        _sim: &[D4],
    ) -> Vec<(f64, Vec<E2>)> {
        unimplemented!();
    }

    fn penalty_per_covering_sym(self: &Self, board: &Board, ppc: &mut Vec<(f64, Vector, Vector)>) {
        let orig_len = ppc.len();

        let c2q = board.cover2_queries_symmetric();
        let ba = board.board_analyzer_symmetric();

        c2q.coverings2()
            .for_each(|(m1, m2)| ppc.push((0.0, *m1, *m2)));

        let ppc = &mut ppc[orig_len..];

        if ppc.is_empty() {
            return;
        }

        ppc.iter_mut().for_each(|(pen, m1, m2)| {
            self.penalize_small_friction(m1, m2, c2q, pen);

            // let depth = c2q.insert_depth_symmetric(*m1 - &*m2);
            // *pen += (self.params.pen_depth_base).powi(depth as i32) * self.params.pen_depth_weight;
        });

        ppc.iter_mut().for_each(|(pen, m1, m2)| {
            self.penalize_shallow_cover(board, m1, m2, ba, pen);
        });

        let i = (0..ppc.len())
            .max_by(|x, y| ppc[*x].0.partial_cmp(&ppc[*y].0).unwrap())
            .unwrap();
        ppc.swap(0, i);
    }

    fn clone(self: &Self) -> Box<dyn Penalizer> {
        Box::new(Self {
            params: self.params.clone(),
            pen_friction_base_powers: self.pen_friction_base_powers.clone(),
            pen_shallowness_weights: self.pen_shallowness_weights.clone(),
            pen_shallowness_diag_weights: self.pen_shallowness_diag_weights.clone(),
        })
    }
}

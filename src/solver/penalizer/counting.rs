use crate::{
    data::{board::Board, d4::D4, e2::E2, vector::Vector},
    solver::parameters::Parameters,
};

use super::Penalizer;

pub struct CountingPenalizer;

impl Penalizer for CountingPenalizer {
    fn init(&mut self, _params: Parameters) {}

    fn penalty_per_covering(
        self: &Self,
        board: &Board,
        _include_other_coverings: bool,
        _sim: &[D4],
    ) -> Vec<(f64, Vec<E2>)> {
        let mut res = vec![];
        for c2 in board.coverings2().iter() {
            res.push((1.0, vec![c2.0, c2.1]));
        }
        res
    }

    fn penalty_per_covering_sym(self: &Self, board: &Board, ppc: &mut Vec<(f64, Vector, Vector)>) {
        board
            .coverings2_symmetric()
            .for_each(|x| ppc.push((1.0, x.0, x.1)))
    }

    fn clone(self: &Self) -> Box<dyn Penalizer> {
        Box::new(Self)
    }
}

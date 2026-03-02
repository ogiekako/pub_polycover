use std::str::FromStr;

use crate::data::{board::Board, d4::D4, e2::E2, vector::Vector};

use super::parameters::Parameters;

pub mod counting;
pub mod simple;
pub mod standard;
pub mod symmetric;

pub trait Penalizer: Send + Sync {
    fn init(&mut self, params: Parameters);

    fn penalty_per_covering(
        &self,
        board: &Board,
        _include_other_coverings: bool,
        sim: &[D4],
    ) -> Vec<(f64, Vec<E2>)>;

    fn penalty_per_covering_sym(&self, board: &Board, ppc: &mut Vec<(f64, Vector, Vector)>);

    fn clone(&self) -> Box<dyn Penalizer>;
}

impl FromStr for Box<dyn Penalizer> {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "standard" => Ok(Box::new(standard::StandardPenalizer::new())),
            "symmetric" => Ok(Box::new(symmetric::SymmetricPenalizer::new())),
            "counting" => Ok(Box::new(counting::CountingPenalizer)),
            "simple" => Ok(Box::new(simple::SimplePenalizer::new())),
            _ => Err(format!("Unknown penalizer: {}", s)),
        }
    }
}

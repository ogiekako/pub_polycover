use log::info;

use crate::{
    data::{tight_poly::TightPoly, vector::Vector},
    solver::sa_solver_symmetric,
};

use super::penalizer::Penalizer;

pub fn solve(
    name: &str,
    problem: &TightPoly,
    side: usize,
    max_side: Option<usize>,
    mut num_iter: usize,
    verbose: bool,
    nproc: usize,
    no_retry: bool,
    retry_on_expand: bool,
    no_give_up: bool,
    penalizer: Box<dyn Penalizer>,
    max_temp: Option<f64>,
    forbidden: usize,
) -> Option<Vec<TightPoly>> {
    let always_include_other_coverings = false;

    let res = sa_solver_symmetric::solve(
        name,
        problem.clone(),
        side,
        num_iter,
        verbose,
        nproc,
        max_temp,
        None,
        no_retry,
        false,
        always_include_other_coverings,
        penalizer.clone(),
        None,
        forbidden,
        None,
    );

    let mut min_cands = match res {
        Ok(sol) => return Some(sol),
        Err(x) => x,
    };
    let (mut penalty, minimum_cand, minimum_cand_params) = min_cands
        .iter()
        .min_by(|x, y| x.0.partial_cmp(&y.0).unwrap())
        .unwrap()
        .clone();
    let mut min_penalty = (penalty, minimum_cand, minimum_cand_params);

    let mut no_improve = 0;
    let mut no_improve_min = 0;

    let mut penalty_history = vec![penalty];

    info!("History: {penalty:.1e} ({})", side);

    let mut exp_side = side - 2;
    while exp_side <= max_side.unwrap_or(usize::MAX) - 2 {
        exp_side += 2;

        if exp_side > side {
            num_iter = (num_iter * exp_side * exp_side) / ((exp_side - 2) * (exp_side - 2));
        }

        let min_cands_with_offset = min_cands
            .iter()
            .map(|(_, cand, _)| {
                (
                    Vector::new(
                        (exp_side - cand.height()) as i32 / 2,
                        (exp_side - cand.width()) as i32 / 2,
                    ),
                    cand.clone(),
                )
            })
            .collect::<Vec<_>>();

        let res = sa_solver_symmetric::solve(
            name,
            problem.clone(),
            exp_side,
            num_iter,
            verbose,
            nproc,
            if exp_side == side {
                max_temp
            } else {
                (penalty * 1.2).into()
            },
            min_cands_with_offset.into(),
            !retry_on_expand,
            false,
            always_include_other_coverings,
            penalizer.clone(),
            (min_penalty.2.clone(), 0.5).into(),
            forbidden + (exp_side - side) / 2,
            None,
        );

        match res {
            Ok(sol) => return Some(sol),
            Err(mut pcs) => {
                pcs.sort_by(|x, y| (x.0).partial_cmp(&y.0).unwrap());

                penalty_history.push(pcs[0].0);

                info!(
                    "History: ({}) {} ({})",
                    exp_side,
                    penalty_history
                        .iter()
                        .rev()
                        .map(|x| format!("{x:.1e}"))
                        .collect::<Vec<_>>()
                        .join(" <- "),
                    side
                );

                if pcs[0].0 < penalty {
                    no_improve = 0;
                } else if pcs[0].0 > penalty {
                    no_improve += 1;
                }

                penalty = pcs[0].0;

                if pcs[0].0 < min_penalty.0 {
                    no_improve_min = 0;
                    min_penalty = pcs[0].clone();
                } else {
                    no_improve_min += 1;
                    pcs.insert(0, min_penalty.clone());
                }

                if !no_give_up && (no_improve >= 3 || no_improve_min >= 6) {
                    // if d4s.len() == 1 {
                    //     no_improve /= 2;
                    //     no_improve_min /= 2;
                    //     min_penalty.0 *= 16.0;
                    //     d4s = vec![D4::I, D4::S0];
                    //     exp_side -= 2;
                    //     continue;
                    // }
                    // else if d4s.len() == 2 {
                    //     d4s = vec![D4::I, D4::R1, D4::S0, D4::S3];
                    //     continue;
                    // }
                    return None;
                }

                min_cands.clear();

                for i in 0..nproc {
                    min_cands.push(pcs[(i + 1).trailing_zeros() as usize].clone());
                }
                assert_eq!(min_cands.len(), nproc);
            }
        }
    }
    None
}

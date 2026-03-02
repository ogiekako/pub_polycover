use crate::{
    check::check::full_check,
    data::{board::Board, d4::D4, tight_poly::TightPoly, vector::Vector},
};

use super::{
    parameters::Parameters, penalizer::counting::CountingPenalizer, sa_solver, sa_solver_symmetric,
};

pub fn trim(
    name: &str,
    problem: &TightPoly,
    solution: &TightPoly,
    extensive: bool,
    symmetric_only: bool,
) -> TightPoly {
    if !extensive {
        return trim_inner(name, problem, solution, None, true, symmetric_only);
    }
    let mut res = solution.clone();
    let mut updated = true;
    for p in [0., 1., 2.] {
        let cand = trim_inner(name, problem, &res, Some(p), updated, symmetric_only);
        updated = cand != res;
        res = cand;
    }
    res
}

fn trim_inner(
    name: &str,
    problem: &TightPoly,
    solution: &TightPoly,
    max_temp_side_power: Option<f64>,
    trim_with_removal: bool,
    symmetric_only: bool,
) -> TightPoly {
    let mut board = Board::new(problem.clone(), solution.height(), solution.width()).unwrap();
    {
        let mut cells = solution.cells();
        while !cells.is_empty() {
            cells.retain(|p| board.try_flip(p.x as usize, p.y as usize).is_err());
        }
    }

    let symmetric_d4s = if symmetric_only {
        [vec![D4::all().collect::<Vec<_>>()], vec![vec![D4::I]]]
    } else {
        [
            vec![
                D4::all().collect::<Vec<_>>(),
                vec![D4::I, D4::R1, D4::R2, D4::R3],
                vec![D4::I, D4::R2],
                vec![D4::I],
            ],
            vec![
                D4::all().collect::<Vec<_>>(),
                vec![D4::I, D4::R1, D4::R2, D4::R3],
                vec![D4::I, D4::R2],
                vec![D4::I],
            ],
        ]
    };

    if trim_with_removal {
        for d4s in symmetric_d4s[0].iter() {
            loop {
                let mut updated = false;
                for x in 0..board.height() {
                    for y in 0..board.width() {
                        let p = Vector::new(x as i32, y as i32);

                        let mut ps = vec![];
                        for d in d4s.iter() {
                            ps.push(board.applied(*d, &p));
                        }
                        ps.sort();
                        ps.dedup();

                        if ps.iter().any(|p| ps[0] > *p) {
                            continue;
                        }
                        ps.retain(|p| board.get(*p));
                        if ps.is_empty() {
                            continue;
                        }

                        let mut ok = true;
                        for i in 0..ps.len() {
                            if board.try_flip(ps[i].x as usize, ps[i].y as usize).is_err() {
                                ok = false;
                                for j in (0..i).rev() {
                                    board.try_flip(ps[j].x as usize, ps[j].y as usize).unwrap();
                                }
                                break;
                            }
                        }
                        if !ok {
                            continue;
                        }

                        if !board.tree().poly().is_empty()
                            && board.coverings2().is_empty()
                            && board.other_coverings().is_empty()
                        {
                            updated = true;
                            continue;
                        }

                        for p in ps.iter().rev() {
                            board.try_flip(p.x as usize, p.y as usize).unwrap();
                        }
                    }
                }
                if !updated {
                    break;
                }
            }
        }
    }

    let mut cand: TightPoly = board.tree().into();

    let Some(power) = max_temp_side_power else {
        return cand;
    };

    let side = board.height().max(board.width());
    let nproc = 2;

    for d4s in symmetric_d4s[1].iter().rev() {
        if d4s.len() == 1 && problem.width().max(problem.height()) > 5 {
            continue;
        }

        let num_iter = 1_000_000;
        let d = d4s.len();
        let num_iter = num_iter / ((d * (d + 1)) / 2);

        let sz = 1. / (side * side) as f64;

        let off = Vector::new(
            (side - cand.height()) as i32 / 2,
            (side - cand.width()) as i32 / 2,
        );
        let mut initial_cand = vec![(off, cand.clone()); nproc];

        if d4s.len() == 1 {
            let num_iter_sym = 200_000;

            let mut params = Parameters::default();
            params.use_restriction_prob = 0.1;
            params.use_tie_prob = 0.001;
            params.act_2x2 = 50;
            params.act_3x3 = 50;
            params.act_flip = 20;
            params.act_clear_to_break = 0;
            params.act_set_to_break = 0;
            params.act_2x2_to_break = 0;
            params.act_3x3_to_break = 0;
            params.act_swap = 5;
            params.act_zip = 5;
            params.act_set = 5;
            params.act_clear = 10;
            params.run_full_check = false;

            if let Ok(better_cand) = sa_solver_symmetric::solve(
                name,
                problem.clone(),
                side,
                num_iter_sym,
                false,
                nproc,
                sz.powf(power).into(),
                initial_cand.clone().into(),
                true,
                true,
                false,
                Box::new(CountingPenalizer),
                (params, 1.0).into(),
                0,
                None,
            ) {
                for better_cand in better_cand {
                    if cand.cells().len() >= better_cand.cells().len()
                        && full_check(&problem, &better_cand).is_ok()
                    {
                        cand = better_cand;
                    }
                }
            }

            let off = Vector::new(
                (side - cand.height()) as i32 / 2,
                (side - cand.width()) as i32 / 2,
            );
            initial_cand = vec![(off, cand.clone()); nproc];
        }

        if let Ok(better_cand) = sa_solver::solve(
            name,
            problem.clone(),
            side,
            num_iter,
            false,
            nproc,
            d4s.clone().into(),
            sz.powf(power).into(),
            initial_cand.into(),
            true,
            true,
            false,
            Box::new(CountingPenalizer),
        ) {
            if cand.cells().len() >= better_cand.cells().len()
                && full_check(&problem, &better_cand).is_ok()
            {
                cand = better_cand;
            }
        }
    }
    cand
}

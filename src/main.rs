#![allow(dead_code)]

use anyhow::Result;
use argopt::{cmd_group, subcmd};
use polycover2::data::board::Board;
use polycover2::data::cover1_queries::Cover1Queries;
use polycover2::data::cover2_queries_symmetric::Cover2QueriesSymmetric;
use polycover2::data::d4::ALL_D4;
use polycover2::data::outline::Outline;
use polycover2::data::tight_poly::TightPoly;
use polycover2::data::u256::U512;
use polycover2::data::vector::Vector;
use polycover2::fs::Client;
use polycover2::profile::Profile;
use polycover2::solver::penalizer::simple::SimplePenalizer;
use polycover2::solver::penalizer::Penalizer;
use polycover2::solver::sa_solver_symmetric;
use polycover2::solver::trim::trim;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;

use std::path::PathBuf;
use std::str::FromStr;
use std::time::Instant;

use polycover2::check::check::check;
use polycover2::{fs, solver};

#[subcmd]
fn bench_check() -> Result<()> {
    let client = fs::Client::new();
    let problems = client.read_problems()?;

    for problem in problems {
        if !problem.solved {
            continue;
        }
        let solution = client.read_solution(&problem)?;

        eprintln!("Checking {}/{}", problem.cell_count, problem.name);

        let res = check(&problem.problem, &solution);

        if res.cover_with_1_count != 0 || res.cover_with_2_count != 0 {
            println!(
                "{} is not a solution: cover_with_1_count = {}, cover_with_2_count = {}",
                problem.name, res.cover_with_1_count, res.cover_with_2_count
            );
        }
    }

    Ok(())
}

#[subcmd]
fn trim_all(
    #[opt(long)] from: Option<String>,
    #[opt(long)] only: Option<String>,
    #[opt(long)] min_side: Option<usize>,
    #[opt(long)] sym: bool,
    #[opt(long)] lightweight: bool,
) -> Result<()> {
    let client = fs::Client::new();
    let mut problems = client
        .read_problems()?
        .into_iter()
        .filter(|p| p.solved)
        .collect::<Vec<_>>();
    problems.sort_by_key(|p| p.problem_name());

    if let Some(from) = from {
        let mut i = 0;
        while i < problems.len() && problems[i].problem_name() != from {
            i += 1;
        }
        problems = problems[i..].to_vec();
    }
    if let Some(only) = only {
        problems.retain(|x| x.problem_name() == only);
    }
    let mut solutions = vec![];
    problems
        .par_iter()
        .map(|p| client.read_solution(p).unwrap())
        .collect_into_vec(&mut solutions);

    for i in (0..problems.len()).rev() {
        if let Some(min_side) = min_side.as_ref() {
            if solutions[i].height().max(solutions[i].width()) < *min_side {
                problems.remove(i);
            }
        }
    }

    problems.into_par_iter().for_each(|problem| {
        let name = problem.problem_name();

        loop {
            let solution = client.read_solution(&problem).unwrap();

            let trimmed = solver::trim::trim(&name, &problem.problem, &solution, !lightweight, sym);

            if !client.write_solution_if_better(&problem, trimmed).unwrap() {
                break;
            }
        }
    });

    Ok(())
}

#[subcmd]
fn bench_full_check() -> Result<()> {
    let _profile = Profile::new();

    let client = fs::Client::new();
    let problems = client.read_problems()?;

    let solved_problems = problems
        .into_iter()
        .filter(|problem| problem.solved)
        .collect::<Vec<_>>();

    solved_problems.into_par_iter().for_each(|problem| {
        let solution = client.read_solution(&problem).unwrap();

        let name = problem.name;

        eprintln!("Checking {}/{}", problem.cell_count, name);

        let mut board =
            Board::new(problem.problem.clone(), solution.height(), solution.width()).unwrap();
        let mut cells = solution.cells();
        while !cells.is_empty() {
            let prev_count = cells.len();
            cells.retain(|p| board.try_flip(p.x as usize, p.y as usize).is_err());

            assert_ne!(
                cells.len(),
                prev_count,
                "{cells:?} {:?}",
                board.try_flip(cells[0].x as usize, cells[0].y as usize)
            );
        }

        assert_eq!(board.coverings2().len(), 0, "{name}");
        assert_eq!(board.other_coverings().len(), 0, "{name}");
    });

    Ok(())
}

#[subcmd]
fn solve(
    #[opt(short, long)] profile: bool,
    #[opt(short, long, default_value_t = 16)] nproc: usize,
    #[opt(long)] mut no_retry: bool,
    #[opt(long)] never_retry: bool,
    #[opt(long)] max_side: Option<usize>,
    #[opt(long)] no_change_side: bool,
    #[opt(long)] no_give_up: bool,
    #[opt(long)] penalizer: Option<String>,
    #[opt(long)] max_temp: Option<f64>,
    #[opt(long, default_value_t = 0)] forbidden: usize,
    path: PathBuf,
    initial_side: usize,
    iter: usize,
) -> Result<()> {
    if never_retry {
        no_retry = true;
    }
    let max_side = if no_change_side {
        Some(initial_side)
    } else {
        max_side.unwrap_or(U512::BITS as usize - 1).into()
    };

    let penalizer = penalizer
        .map(|x| x.parse::<Box<dyn Penalizer>>().unwrap())
        .unwrap_or_else(|| Box::new(SimplePenalizer::new()));

    let _profile = if profile { Some(Profile::new()) } else { None };

    let client = fs::Client::new();
    let problem = client.parse_problem(path)?;

    let mut has_incremented = false;
    let mut found_in_current_run = false;
    let mut side = initial_side;
    loop {
        if side != initial_side && no_change_side {
            break;
        }
        let solution = solver::expand_sa_solver::solve(
            &problem.problem_name(),
            &problem.problem,
            side,
            max_side,
            iter,
            true,
            nproc,
            no_retry,
            !never_retry,
            no_give_up,
            penalizer.clone(),
            max_temp,
            forbidden,
        );

        if let Some(solutions) = solution {
            found_in_current_run = true;

            let mut better = false;
            for solution in solutions.iter() {
                eprintln!("Solution found!\n{}", solution);
                better |= client.write_solution_if_better(&problem, solution.clone())?;

                let trimmed = trim(
                    &problem.problem_name(),
                    &problem.problem,
                    &solution,
                    false,
                    true,
                );
                eprintln!("Trimmed solution\n{}", trimmed);

                better |= client.write_solution_if_better(&problem, trimmed)?;
            }

            side = solutions[0].height().max(solutions[0].width()).min(side) - 2;
            if side < 7 || has_incremented || !better {
                break;
            }
            eprintln!("Trying to find a better solution with side = {side}");
        } else {
            if found_in_current_run {
                eprintln!("Solution not found");
                break;
            }

            if let Some(max_side) = max_side {
                if side >= max_side {
                    break;
                }
                has_incremented = false;
                side += 2;
                eprintln!("Making another try with side = {side}");
            } else {
                eprintln!("Solution not found");
                break;
            }
        }
    }

    Ok(())
}

#[subcmd]
fn solve_all(
    #[opt(short, long)] seed: Option<u64>,
    #[opt(short, long)] unsolved_only: bool,
    #[opt(short, long, default_value_t = 16)] nproc: usize,
    #[opt(long)] no_verbose: bool,
    #[opt(long)] from: Option<String>,
    #[opt(long)] no_smaller: bool,
    #[opt(long)] no_unsolved: bool,
    #[opt(long)] mut no_retry: bool,
    #[opt(long)] never_retry: bool,
    #[opt(long)] no_give_up: bool,
    #[opt(long)] penalizer: Option<String>,
    #[opt(long)] max_temp: Option<f64>,
    #[opt(long, default_value_t = 0)] forbidden: usize,
    #[opt(long)] sub1: bool,
    max_side: usize,
    iter: usize,
) -> Result<()> {
    if never_retry {
        no_retry = true;
    }

    let penalizer = penalizer
        .map(|x| x.parse::<Box<dyn Penalizer>>().unwrap())
        .unwrap_or_else(|| Box::new(SimplePenalizer::new()));

    let client = fs::Client::new();
    let mut problems = client.read_problems()?.into_iter().collect::<Vec<_>>();
    if let Some(seed) = seed {
        problems.shuffle(&mut SmallRng::seed_from_u64(seed));
    } else {
        problems.sort_by_key(|p| p.problem_name());
    }

    let mut started = from.is_none();
    for problem in problems.iter() {
        let name = problem.problem_name();

        if !started && from.as_ref().unwrap().contains(name.as_str()) {
            started = true;
        }
        if !started {
            continue;
        }

        if unsolved_only && problem.solved {
            continue;
        }
        if no_unsolved && !problem.solved {
            continue;
        }

        let best_solution = if problem.solved {
            Some(client.read_solution(&problem)?)
        } else {
            None
        };

        let start_side = best_solution
            .as_ref()
            .map(|s| (s.height().max(s.width()) - if sub1 { 1 } else { 2 }).min(max_side))
            .unwrap_or(max_side);

        if start_side < max_side && no_smaller {
            continue;
        }
        let mut max_side = best_solution
            .map(|s| s.height().max(s.width()))
            .unwrap_or(U512::BITS as usize - 1);

        for side in (1..=start_side).rev().step_by(2) {
            let solution = solver::expand_sa_solver::solve(
                &name,
                &problem.problem,
                side,
                max_side.into(),
                iter,
                !no_verbose,
                nproc,
                no_retry,
                !never_retry,
                no_give_up,
                penalizer.clone(),
                max_temp,
                forbidden,
            );

            if let Some(solutions) = solution {
                let mut better = false;
                for solution in solutions.iter() {
                    eprintln!("Solution found!\n{}", solution);
                    better |= client.write_solution_if_better(&problem, solution.clone())?;

                    let trimmed = trim(
                        &problem.problem_name(),
                        &problem.problem,
                        &solution,
                        false,
                        true,
                    );
                    eprintln!("Trimmed solution\n{}", trimmed);

                    better |= client.write_solution_if_better(&problem, trimmed)?;
                }
                if !better {
                    break;
                }

                max_side = solutions[0].height().max(solutions[0].width());
            } else {
                eprintln!("Solution not found");
                break;
            }
        }
    }
    Ok(())
}

#[subcmd]
fn tidy() -> Result<()> {
    let cl = Client::default();
    let problems = cl.read_problems()?;
    let mut can_remove = vec![false; problems.len()];

    for (i, problem) in problems.iter().enumerate() {
        if !problem.solved {
            continue;
        }
        for d in ALL_D4.iter() {
            let problem = problem.problem.apply(*d);
            for (j, other) in problems.iter().enumerate() {
                if i == j {
                    continue;
                }
                if let Err(e) = Board::new_with_initial_cand(problem.clone(), &other.problem, false)
                {
                    if e.to_string().contains("cand contains problem") {
                        // other covers problem.
                        can_remove[j] = true;
                    }
                }
            }
        }
    }

    for (i, problem) in problems.iter().enumerate() {
        if !can_remove[i] {
            continue;
        }
        cl.remove_problem_and_solution(problem)?;
    }

    Ok(())
}

#[subcmd]
fn bench_cover2(#[opt(short, long)] profile: bool) -> Result<()> {
    let _profile = if profile { Some(Profile::new()) } else { None };

    let problem = TightPoly::from_str("3 3\n###\n#..\n#..\n").unwrap();

    let side = 101;
    let depth = 10;
    let outline = Outline::new(side, side);

    let num_iter = 4;

    (0..num_iter).into_iter().for_each(|_| {
        let mut seen = vec![-1i32; side * side];

        let mut rng = SmallRng::seed_from_u64(0);

        let first = 5000;
        let n = 95000;

        let mut total_cov2 = 0;
        let mut coverings2 = vec![];

        let mut cov1 = Cover1Queries::new(problem.clone(), side);
        let mut cov2 = Cover2QueriesSymmetric::new(problem.clone(), side, depth);

        let mut start = Instant::now();
        for i in 0..n + first {
            if i == first {
                start = Instant::now();
                total_cov2 = 0;
            }

            let mut ps = vec![];

            for _ in 0..6 {
                let p = Vector::new(
                    rng.gen::<i32>().abs() % side as i32,
                    rng.gen::<i32>().abs() % side as i32,
                );
                ps.push(p);
            }

            let mut to_flip = vec![];
            for p in ps {
                for j in 0..8 {
                    let (x, y) = outline
                        .cell(p.x as usize, p.y as usize)
                        .transform(ALL_D4[j]);

                    assert!(x < side && y < side);

                    if seen[x * side + y] == i {
                        continue;
                    }
                    to_flip.push(Vector::new(x as i32, y as i32));
                    seen[x * side + y] = i;
                }
            }

            for p in to_flip.iter().copied() {
                cov1.flip(p);
            }
            if cov1.coverings1().is_empty() {
                cov2.flip(&to_flip);
                coverings2 = cov2.coverings2().collect();
            } else {
                for p in to_flip.iter().copied().rev() {
                    cov1.flip(p);
                }
            }

            total_cov2 += coverings2.len();

            if i == n + first - 1 {
                let speed = n as f64 / start.elapsed().as_secs_f64();

                println!(
                    "ave cov2: {:.1}, speed: {:.1} (iter/s)",
                    total_cov2 as f64 / n as f64,
                    speed
                );
            }
        }
    });

    Ok(())
}

#[subcmd]
fn bench_sa(
    #[opt(short, long)] profile: bool,
    #[opt(long)] expect: Option<f64>,
    #[opt(long)] penalizer: Option<String>,
) -> Result<()> {
    let penalizer = penalizer
        .map(|x| x.parse::<Box<dyn Penalizer>>().unwrap())
        .unwrap_or_else(|| Box::new(SimplePenalizer::new()));

    let problem = TightPoly::from_str("3 3\n###\n#..\n#..\n").unwrap();

    sa_solver_symmetric::init(&problem);

    let _profile = if profile { Some(Profile::new()) } else { None };

    let side = 127;

    let num_iter = 3;

    let base_seed = Some(50127);

    (0..num_iter).into_iter().for_each(|_| {
        let n = 100000;

        let start = Instant::now();

        let penalty = match sa_solver_symmetric::solve(
            "bench",
            problem.clone(),
            side,
            n,
            false,
            2,
            None,
            None,
            true,
            false,
            false,
            penalizer.clone(),
            None,
            0,
            base_seed,
        ) {
            Ok(_) => 0.,
            Err(p) => p
                .into_iter()
                .map(|x| x.0)
                .min_by(|x, y| x.partial_cmp(y).unwrap())
                .unwrap(),
        };

        let speed = n as f64 / start.elapsed().as_secs_f64();
        println!("penalty: {:.1} speed: {:.1} (iter/s)", penalty, speed);

        if let Some(expect) = expect {
            assert_eq!(penalty.floor(), expect);
        }
    });

    Ok(())
}

#[subcmd]
fn research() -> Result<()> {
    for n in 1..8 {
        for m in n.. {
            let mut v = vec![m];
            if recur(&mut v, n, 0) {
                println!("{}", m);
                break;
            }
        }
    }
    Ok(())
}

fn recur(v: &mut Vec<usize>, n: usize, mask: usize) -> bool {
    if v.len() == n {
        return true;
    }
    for s in (1..*v.last().unwrap()).rev() {
        let mut mask = mask;

        let mut valid = true;
        for &t in v.iter().rev() {
            let i = t - s;
            if mask >> i & 1 == 1 {
                valid = false;
            }
            mask |= 1 << i;
        }
        if !valid {
            continue;
        }

        v.push(s);
        if recur(v, n, mask) {
            return true;
        }
        v.pop();
    }
    false
}

#[allow(non_camel_case_types)]
#[cmd_group(commands = [bench_check, bench_full_check, solve, trim_all, solve_all, tidy, bench_cover2, bench_sa, research])]
#[opt(author, version, about, long_about = None)]
fn main() -> Result<()> {
    env_logger::init();
}

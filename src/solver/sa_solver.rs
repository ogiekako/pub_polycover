use std::{
    collections::BTreeMap,
    f64::consts::E,
    ops::Range,
    str::FromStr,
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};

use anyhow::{anyhow, Context, Result};
use colored::*;
use log::{error, info};
use rand::{rngs::SmallRng, Rng, SeedableRng};
use rayon::prelude::*;

use crate::data::{
    board::Board, connection::Connections, d4::D4, e2::E2, rect::Rect, tight_poly::TightPoly,
    vector::Vector,
};

use super::penalizer::Penalizer;

static CONNECTIONS3: OnceLock<Mutex<BTreeMap<TightPoly, Arc<Connections>>>> = OnceLock::new();

pub fn init(problem: &TightPoly) {
    connecitons3(problem);
}

fn connecitons3(problem: &TightPoly) -> Arc<Connections> {
    let mut g = CONNECTIONS3
        .get_or_init(|| Default::default())
        .lock()
        .unwrap();

    g.entry(problem.clone())
        .or_insert_with(|| Arc::new(Connections::new_with_problem(3, problem)))
        .clone()
}

pub fn solve(
    name: &str,
    problem: TightPoly,
    side: usize,
    num_iter: usize,
    verbose: bool,
    nproc: usize,
    d4s: Option<Vec<D4>>,
    max_temp: Option<f64>,
    initial_cand: Option<Vec<(Vector, TightPoly)>>,
    no_retry: bool,
    no_early_return: bool,
    always_include_other_coverings: bool,
    penalizer: Box<dyn Penalizer>,
) -> Result<TightPoly, (Vec<(f64, TightPoly)>, Vec<D4>)> {
    if initial_cand.is_none() {
        assert!(num_iter > 25_000);
    }

    let connections3 = connecitons3(&problem);

    if d4s.is_none() {
        assert!(initial_cand.is_none());

        let mut solver = Solver::new(
            problem.clone(),
            11,
            10_000,
            0,
            false,
            Arc::new(Mutex::new((f64::MAX, 0))),
            vec![D4::I],
            None,
            None,
            false,
            false,
            connections3,
            penalizer.clone(),
        )
        .unwrap();
        let res = solver.solve(true);
        return match res {
            Ok(res) => Ok(res),
            Err((penalty, board)) => {
                let mut initial_cand = None;
                if *penalty >= INITIAL_TEMP
                    || board
                        .bounding_box()
                        .height()
                        .min(board.bounding_box().width())
                        <= 5
                {
                    info!("Try semi-symmetric solution: penalty = {:.1}", penalty);

                    let off = Vector::new(side as i32 / 2 - 2, side as i32 / 2 - 2);
                    let cand = TightPoly::from_str(
                        r#"5 5
.###.
##.##
#...#
...##
..##.
"#,
                    )
                    .unwrap();

                    initial_cand = Some(vec![(off, cand); nproc]);
                }
                return solve(
                    name,
                    problem,
                    side,
                    num_iter,
                    verbose,
                    nproc,
                    vec![D4::I].into(),
                    max_temp,
                    initial_cand,
                    no_retry,
                    no_early_return,
                    always_include_other_coverings,
                    penalizer,
                );
            }
        };
    }

    if verbose {
        info!("Solving {} with side {} and iter {}", name, side, num_iter);
    }

    let seeds: Vec<_> = (0..nproc as u64)
        .map(|x| x + (side + num_iter) as u64)
        .collect();

    let shared_min: Arc<Mutex<(f64, u64)>> = Arc::new(Mutex::new((f64::MAX, 0)));

    let results = seeds
        .par_iter()
        .map(|&seed| {
            let mut solver = Solver::new(
                problem.clone(),
                side,
                num_iter,
                seed,
                verbose,
                shared_min.clone(),
                d4s.as_ref().unwrap().clone(),
                max_temp,
                initial_cand
                    .as_ref()
                    .map(|x| x[seed as usize % x.len()].clone()),
                no_early_return,
                always_include_other_coverings,
                connections3.clone(),
                penalizer.clone(),
            )
            .map_err(|_| (f64::INFINITY, TightPoly::from_str("0 0").unwrap()))?;
            let res = solver.solve(no_retry);
            if res.is_ok() {
                *shared_min.lock().unwrap() = (0.0, seed);

                if verbose {
                    info!("Found solution with seed {}", seed);
                }
            }
            res.map_err(|x| (x.0, TightPoly::from(x.1.tree())))
        })
        .collect::<Vec<_>>();

    if results.iter().any(|x| x.is_ok()) {
        Ok(results.into_iter().find(|x| x.is_ok()).unwrap().unwrap())
    } else {
        let min_cands = results
            .iter()
            .map(|x| x.as_ref().unwrap_err().clone())
            .collect::<Vec<_>>();

        if verbose {
            info!(
                "Minimum penalty: {:.2}",
                min_cands
                    .iter()
                    .map(|x| x.0)
                    .min_by(|x, y| x.partial_cmp(&y).unwrap())
                    .unwrap()
            );
        }
        Err((min_cands, d4s.unwrap()))
    }
}

struct Solver {
    seed: u64,

    board: Board,
    iteration: usize,
    rng: SmallRng,

    // penalty per covering
    ppc: Vec<(f64, Vec<E2>)>,
    penalty: f64,
    min_penalty: (f64, Board),

    verbose: bool,

    shared_min: Arc<Mutex<(f64, u64)>>,
    winning: bool,

    sim: Vec<D4>,

    retry_threshold: usize,
    no_improve_count: usize,

    initial_temp: Option<f64>,
    min_temp: f64,
    max_temp: f64,

    no_early_return: bool,
    always_include_other_coverings: bool,

    connections2: Arc<Connections>,
    connections3: Arc<Connections>,

    start_time: Instant,

    penalizer: Box<dyn Penalizer>,
}

const INITIAL_RETRY_THRESHOLD: usize = 50_000;

const INITIAL_TEMP: f64 = 1_000_000.0;
const MAX_TEMP: f64 = 5.0;
const MIN_TEMP: f64 = 0.0;

impl Solver {
    fn new(
        problem: TightPoly,
        side: usize,
        iteration: usize,
        seed: u64,
        verbose: bool,
        shared_min: Arc<Mutex<(f64, u64)>>,
        d4s: Vec<D4>,
        max_temp: Option<f64>,
        initial_cand: Option<(Vector, TightPoly)>,
        no_early_return: bool,
        always_include_other_coverings: bool,
        connections3: Arc<Connections>,
        penalizer: Box<dyn Penalizer>,
    ) -> Result<Self> {
        let rng = SmallRng::seed_from_u64(
            seed + {
                if let Some(c) = initial_cand.as_ref() {
                    c.1.cells().len() as u64
                } else {
                    0
                }
            },
        );
        let mut board = Board::new_with_allowed_d4s(
            problem.clone(),
            side,
            side,
            d4s.clone(),
            initial_cand.clone(),
            5,
        )?;

        if initial_cand.is_none() {
            board.start_transaction();
            board.try_flip(side / 2, side / 2).unwrap();
            board.commit_transaction();
        }

        let mut sim = vec![];
        let mut mask = 0;
        for d in D4::all() {
            let i = d as usize;
            if mask >> i & 1 != 0 {
                continue;
            }
            sim.push(d);
            for e in d4s.iter() {
                let i = 1 << (*e * d) as usize;
                assert!(mask & i == 0);
                mask |= i;
            }
        }
        assert_eq!(sim.len() * d4s.len(), 8);

        let ppc = penalizer.penalty_per_covering(&board, true, &sim);
        let penalty = penalty(&board, &ppc);

        Ok(Self {
            seed,
            min_penalty: (penalty, board.clone()),
            board,
            iteration,
            rng,
            ppc,
            penalty,
            shared_min,
            winning: false,
            verbose,
            sim,
            retry_threshold: INITIAL_RETRY_THRESHOLD,
            no_improve_count: 0,
            initial_temp: if max_temp.is_some() {
                None
            } else {
                Some(INITIAL_TEMP)
            },
            min_temp: MIN_TEMP,
            max_temp: max_temp.unwrap_or(MAX_TEMP),
            no_early_return,
            always_include_other_coverings,
            connections2: Connections::new_with_problem(2, &problem).into(),
            connections3,
            start_time: Instant::now(),
            penalizer,
        })
    }

    fn solve(&mut self, no_retry: bool) -> Result<TightPoly, &(f64, Board)> {
        let mut iter = 0;
        while iter < self.iteration {
            iter += 1;
            if iter % 50_000 == 0 || iter >= self.iteration {
                if self.penalty <= 0.0 && (iter == self.iteration || !self.no_early_return) {
                    if self.min_penalty.0 < self.penalty {
                        self.board = self.min_penalty.1.clone();
                        self.ppc =
                            self.penalizer
                                .penalty_per_covering(&self.board, true, &self.sim);
                        self.penalty = self.min_penalty.0;
                    }
                    return Ok(self.board.tree().into());
                }

                let mut sm = self.shared_min.lock().unwrap();
                if sm.0 <= 0.0 && !self.no_early_return {
                    return Err(&self.min_penalty);
                }
                if sm.0 > self.min_penalty.0 {
                    *sm = (self.min_penalty.0, self.seed);
                }
                if *sm == (self.min_penalty.0, self.seed) {
                    self.winning = true;
                    if self.verbose {
                        self.print_state(iter);
                    }
                } else {
                    self.winning = false;
                }
            }

            let updated = self.step(iter);

            let use_tie = updated && self.rng.gen_range(0.0..1.0) < 0.1;

            let strictly_improved = self.penalty < self.min_penalty.0;

            // if !self.always_include_other_coverings
            //     && (strictly_improved || self.penalty == self.min_penalty.0 && use_tie)
            // {
            //     self.ppc = penalty_per_covering(&self.board, true, &self.sim);
            //     self.penalty = penalty(&self.board, &self.ppc);

            //     if self.penalty > self.min_penalty.0 {
            //         self.board = self.min_penalty.1.clone();
            //         self.ppc = penalty_per_covering(&self.board, true, &self.sim);
            //         self.penalty = self.min_penalty.0;
            //         strictly_improved = false;
            //         use_tie = false;
            //     }
            // }

            if strictly_improved || self.penalty == self.min_penalty.0 && use_tie {
                let bb = self.board.bounding_box();
                if bb.height() >= self.board.height() / 2 && bb.width() >= self.board.width() / 2 {
                    self.no_improve_count = 0;
                    self.retry_threshold = (self.retry_threshold / 4).max(INITIAL_RETRY_THRESHOLD);

                    self.min_penalty = (self.penalty, self.board.clone());
                }
            }
            if !strictly_improved && !no_retry {
                self.no_improve_count += 1;
                if self.no_improve_count >= self.retry_threshold {
                    self.no_improve_count = 0;
                    iter -= self.retry_threshold;
                    self.retry_threshold *= 4;
                    self.board = self.min_penalty.1.clone();
                    self.ppc = self
                        .penalizer
                        .penalty_per_covering(&self.board, true, &self.sim);
                    self.penalty = self.min_penalty.0;
                    if self.verbose && self.winning {
                        self.print_state(iter);
                        info!("Reset to best state");
                    }
                }
            }
        }
        self.board = self.min_penalty.1.clone();
        self.ppc = self
            .penalizer
            .penalty_per_covering(&self.board, true, &self.sim);
        self.penalty = self.min_penalty.0;
        if self.winning && self.verbose {
            self.print_state(iter);
        }
        Err(&self.min_penalty)
    }

    fn step(&mut self, iter: usize) -> bool {
        let action = self.random_action();

        self.board.start_transaction();
        let failed = action.try_apply(&mut self.board, false).is_err();

        self.board.commit_transaction();

        if failed {
            return false;
        }

        let ppc = self.penalizer.penalty_per_covering(
            &self.board,
            self.always_include_other_coverings,
            &self.sim,
        );
        let new_penalty = penalty(&self.board, &ppc);

        if self.accept(iter, new_penalty) {
            self.ppc = ppc;
            self.penalty = new_penalty;

            return true;
        }

        self.board.start_transaction();
        action.undo(&mut self.board);
        self.board.commit_transaction();

        debug_assert!({
            let ppc = self.penalizer.penalty_per_covering(
                &self.board,
                self.always_include_other_coverings,
                &self.sim,
            );
            let diff = penalty(&self.board, &ppc) - self.penalty;

            let ok = diff.abs() < 1e-3;

            if !ok {
                error!("diff: {}", diff);
            }

            ok
        });

        return false;
    }

    fn random_action(&mut self) -> Action {
        let bb = self.board.bounding_box();

        loop {
            let use_restriction = self.penalty > 0. && self.rng.gen_range(0.0..1.0) < 0.95;

            let allowed_dists = use_restriction.then(|| {
                let r = ((bb.area() as f64).sqrt() as i32) / 2;

                let too_small = (self.rng.gen_range(0..r * r + 1) as f64).sqrt() as i32;
                let too_large = 2 * r + 2 - (self.rng.gen_range(0..r * r + 1) as f64).sqrt() as i32;

                too_small + 1..too_large

                // let r2 = r + 1;
                // let dr = 2 * r2 - r + 1;
                // let a = 2 * r2 - (self.rng.gen_range(0..dr * dr) as f64).sqrt() as i32;
                // let b = 2 * r2 - (self.rng.gen_range(0..dr * dr) as f64).sqrt() as i32;
                // if a <= b {
                //     a..b + 1
                // } else {
                //     b..a + 1
                // }
            });

            for _ in 0..10 {
                if let Some(action) = self.random_action_inner(allowed_dists.clone()) {
                    return action;
                }
            }
        }
    }

    fn random_action_inner(&mut self, allowed_dists: Option<Range<i32>>) -> Option<Action> {
        let center = Vector::new(
            self.board.height() as i32 / 2,
            self.board.width() as i32 / 2,
        );
        let sim_len = self.sim.len();
        let is_forbidden = |x| {
            if x == center && sim_len < 8 {
                return true;
            }
            if let Some(allowed_dists) = allowed_dists.as_ref() {
                let d: Vector = x - center;
                let dist = d.x.abs() + d.y.abs();
                !allowed_dists.contains(&dist)
            } else {
                false
            }
        };

        match self.rng.gen_range(0..1000) {
            // Flip a cell
            0..=99 => {
                let bb = self.board.bounding_box();
                let (min, max) = (bb.min_corner(), bb.max_corner());

                for _ in 0..(bb.area() / 100).max(10) {
                    let v = Vector::new(
                        self.rng.gen_range(min.x - 1..=max.x + 1),
                        self.rng.gen_range(min.y - 1..=max.y + 1),
                    );
                    if is_forbidden(v) {
                        continue;
                    }
                    if !self.board.can_flip(v) {
                        continue;
                    }
                    return self.flip_action(v).into();
                }
            }
            // // Clear a cell
            // 100..=109 => {
            //     let leaves = self.board.leaves();
            //     if leaves.len() <= 1 {
            //         return None;
            //     }

            //     for _ in 0..3 {
            //         let cell = leaves[self.rng.gen_range(0..leaves.len())];

            //         if is_forbidden(cell) {
            //             continue;
            //         }

            //         return self.flip_action(cell).into();
            //     }
            // }
            // // Set a cell
            // 200..=209 => {
            //     let cells = self.board.cells();
            //     for _ in 0..100 {
            //         let cell = cells[self.rng.gen_range(0..cells.len())];
            //         let d = self.rng.gen_range(0..4);
            //         let v = Vector::new(cell.x + DX[d], cell.y + DY[d]);

            //         if self.board.can_set_v(v) && !is_forbidden(v) {
            //             return self.flip_action(v).into();
            //         }
            //     }
            // }
            // Swap
            300..=319 => {
                let bb = self.board.bounding_box();
                let (min, max) = (bb.min_corner(), bb.max_corner());

                for _ in 0..(bb.area() / 100).max(10) {
                    let v = Vector::new(
                        self.rng.gen_range(min.x..=max.x),
                        self.rng.gen_range(min.y..=max.y),
                    );
                    if is_forbidden(v) {
                        continue;
                    }

                    let swap_cands = self.board.substitutables_for(v);
                    if swap_cands.is_empty() {
                        continue;
                    }

                    let u = swap_cands[self.rng.gen_range(0..swap_cands.len())];

                    if self.board.rot180(&u) == u && self.sim.len() < 8 {
                        continue;
                    }

                    if let Some(sw) = self.swap_action(v, u) {
                        return sw.into();
                    }
                }
            }
            // Clear a cell to break a covering
            400..=409 => {
                let mut cov2 = self
                    .ppc
                    .iter()
                    .cloned()
                    .filter(|x| x.1.len() == 2)
                    .collect::<Vec<_>>();
                if cov2.is_empty() {
                    return None;
                }
                for i in 0..cov2.len() - 1 {
                    cov2[i + 1].0 += cov2[i].0;
                }
                let sum = cov2[cov2.len() - 1].0;

                for _ in 0..10 {
                    let r = self.rng.gen_range(0.0..sum);

                    let i = cov2
                        .binary_search_by(|x| x.0.partial_cmp(&r).unwrap())
                        .err()
                        .unwrap();
                    let cov = &cov2[i];

                    let (m1, m2) = (cov.1[0], cov.1[1]);
                    let (m1, m2) = self.rng.gen::<bool>().then(|| (m1, m2)).unwrap_or((m2, m1));

                    let cells = self.board.problem_cells();
                    let p = cells[self.rng.gen_range(0..cells.len())];

                    let (a, b) = (&m1.inverse() * &p, &m2.inverse() * &p);

                    let v = if self.board.get(a) { a } else { b };

                    if is_forbidden(v) {
                        continue;
                    }

                    if self.board.can_clear_v(v) {
                        return self.flip_action(v).into();
                    }

                    let to_set = self.board.substitutables_for(v);
                    if to_set.is_empty() {
                        continue;
                    }
                    let u = to_set[self.rng.gen_range(0..to_set.len())];
                    if let Some(sw) = self.swap_action(v, u) {
                        return sw.into();
                    }
                }
            }
            // Set a cell to break a covering
            500..=519 => {
                let mut cov2 = self
                    .ppc
                    .iter()
                    .cloned()
                    .filter(|x| x.1.len() == 2)
                    .collect::<Vec<_>>();
                if cov2.is_empty() {
                    return None;
                }
                for i in 0..cov2.len() - 1 {
                    cov2[i + 1].0 += cov2[i].0;
                }
                let sum = cov2[cov2.len() - 1].0;

                let bb = {
                    let bb = self.board.bounding_box();
                    let (mut min, mut max) = (bb.min_corner(), bb.max_corner());
                    min = (min - Vector::new(1, 1)).pairwise_max(&Vector::new(0, 0));
                    max = (max + Vector::new(1, 1)).pairwise_min(&Vector::new(
                        self.board.height() as i32 - 1,
                        self.board.width() as i32 - 1,
                    ));
                    Rect::from_min_max(min, max)
                };

                for _ in 0..(bb.area() / 100).max(10) {
                    let r = self.rng.gen_range(0.0..sum);

                    let i = cov2
                        .binary_search_by(|x| x.0.partial_cmp(&r).unwrap())
                        .err()
                        .unwrap();
                    let cov = &cov2[i];

                    let (m1, m2) = (cov.1[0], cov.1[1]);
                    let (m1, m2) = self.rng.gen::<bool>().then(|| (m1, m2)).unwrap_or((m2, m1));

                    let r = (m1 * bb).intersection(&(m2 * bb));
                    if r.is_empty() {
                        continue;
                    }
                    let min = r.min_corner();
                    let max = r.max_corner();
                    let p = Vector::new(
                        self.rng.gen_range(min.x..=max.x),
                        self.rng.gen_range(min.y..=max.y),
                    );
                    let p1 = m1.inverse() * p;
                    let p2 = m2.inverse() * p;

                    let v = if p1 == p2 {
                        p1
                    } else if self.board.get(p1) && !self.board.get(p2) {
                        p2
                    } else if !self.board.get(p1) && self.board.get(p2) {
                        p1
                    } else {
                        continue;
                    };

                    if is_forbidden(v) {
                        continue;
                    }

                    if self.board.can_set_v(v) {
                        return self.flip_action(v).into();
                    }

                    let to_remove = self.board.substitutables_for(v);
                    if to_remove.is_empty() {
                        continue;
                    }
                    let u = to_remove[self.rng.gen_range(0..to_remove.len())];
                    if let Some(sw) = self.swap_action(v, u) {
                        return sw.into();
                    }
                }
            }
            // Zip
            600..=609 => {
                let bb = self.board.bounding_box();
                let (min, max) = (bb.min_corner(), bb.max_corner());
                for _ in 0..(bb.area() / 100).max(10) {
                    let v = Vector::new(
                        self.rng.gen_range(min.x..=max.x),
                        self.rng.gen_range(min.y..=max.y),
                    );

                    if is_forbidden(v) {
                        continue;
                    }

                    if !self.board.can_zip(v) {
                        continue;
                    }

                    return self.zip_action(v).into();
                }
            }
            // Mass flip 2x2, 3x3
            num @ ((700..=709) | (800..=804)) => {
                let s = if num < 800 { 2 } else { 3 };

                let bb = self.board.bounding_box();
                let (mut min, mut max) = (bb.min_corner(), bb.max_corner());
                min -= Vector::new(s - 1, s - 1);
                min = min.pairwise_max(&Vector::new(0, 0));
                max = max.pairwise_min(&Vector::new(
                    self.board.height() as i32 - s,
                    self.board.width() as i32 - s,
                ));
                for _ in 0..(bb.area() / 100).max(10) {
                    let offset = Vector::new(
                        self.rng.gen_range(min.x..=max.x),
                        self.rng.gen_range(min.y..=max.y),
                    );

                    if is_forbidden(offset) {
                        continue;
                    }

                    if let Some(a) = self.make_mass_flip_on(offset, s as usize) {
                        return a.into();
                    }
                }
            }
            // Mass flip to break a covering
            num @ (900..=909 | 950..=954) => {
                let s = if num < 950 { 2 } else { 3 };

                let mut cov2 = self
                    .ppc
                    .iter()
                    .cloned()
                    .filter(|x| x.1.len() == 2)
                    .collect::<Vec<_>>();
                if cov2.is_empty() {
                    return None;
                }

                for i in 0..cov2.len() - 1 {
                    cov2[i + 1].0 += cov2[i].0;
                }
                let sum = cov2[cov2.len() - 1].0;

                let bb = {
                    let bb = self.board.bounding_box();
                    let (mut min, mut max) = (bb.min_corner(), bb.max_corner());
                    min = (min - Vector::new(s, s)).pairwise_max(&Vector::new(0, 0));
                    max = (max + Vector::new(s, s)).pairwise_min(&Vector::new(
                        self.board.height() as i32 - 1,
                        self.board.width() as i32 - 1,
                    ));
                    Rect::from_min_max(min, max)
                };
                let (bb_min, bb_max) = (bb.min_corner(), bb.max_corner());

                for _ in 0..(bb.area() / 100).max(10) {
                    let r = self.rng.gen_range(0.0..sum);

                    let i = cov2
                        .binary_search_by(|x| x.0.partial_cmp(&r).unwrap())
                        .err()
                        .unwrap();
                    let cov = &cov2[i];

                    let (m1, m2) = (cov.1[0], cov.1[1]);
                    let (m1, m2) = self.rng.gen::<bool>().then(|| (m1, m2)).unwrap_or((m2, m1));

                    let r = (m1 * bb).intersection(&(m2 * bb));
                    if r.is_empty() {
                        continue;
                    }
                    let min = r.min_corner();
                    let max = r.max_corner();
                    let p = Vector::new(
                        self.rng.gen_range(min.x - s + 1..=max.x),
                        self.rng.gen_range(min.y - s + 1..=max.y),
                    );
                    let area = Rect::from_min_max(p, p + Vector::new(s - 1, s - 1));

                    let mut offset = (m1.inverse() * area).min_corner();

                    offset = offset.pairwise_max(&bb_min);
                    offset = offset.pairwise_min(&(bb_max - Vector::new(s - 1, s - 1)));

                    if is_forbidden(offset) {
                        continue;
                    }

                    let o1 =
                        Rect::from_min_max(offset - Vector::new(1, 1), offset + Vector::new(s, s));
                    let o2 = m2.inverse() * (m1 * o1);

                    let mut ok = true;
                    'outer: for o in [o1, o2] {
                        let (min, max) = (o.min_corner(), o.max_corner());
                        for x in min.x + 1..max.x {
                            if self.board.get(Vector::new(x, min.y))
                                || self.board.get(Vector::new(x, max.y))
                            {
                                continue 'outer;
                            }
                        }
                        for y in min.y + 1..max.y {
                            if self.board.get(Vector::new(min.x, y))
                                || self.board.get(Vector::new(max.x, y))
                            {
                                continue 'outer;
                            }
                        }
                        ok = false;
                        break;
                    }
                    if !ok {
                        continue;
                    }

                    if let Some(a) = self.make_mass_flip_on(offset, s as usize) {
                        return a.into();
                    }
                }
            }
            _ => (),
        };
        None
    }

    fn make_mass_flip_on(&mut self, offset: Vector, side: usize) -> Option<Action> {
        if self.mass_flip_action(offset, 0, side).is_none() {
            return None;
        }

        let mask_2d = self
            .board
            .tree()
            .mask_2d(offset - Vector::new(1, 1), side + 2);

        let conn = if side == 2 {
            &self.connections2
        } else {
            &self.connections3
        };

        let cands = conn.candidates(mask_2d);

        if cands.len() < 2 {
            return None;
        }

        let mut orig_mask = 0;
        for x in 0..side {
            for y in 0..side {
                if self.board.get(offset + Vector::new(x as i32, y as i32)) {
                    orig_mask |= 1 << (x * side + y);
                }
            }
        }

        for _ in 0..4 {
            let mask = orig_mask ^ cands[self.rng.gen_range(0..cands.len())];
            if mask == 0 {
                continue;
            }
            if let Some(a) = self.mass_flip_action(offset, mask, side) {
                return Some(a);
            }
        }
        None
    }

    fn accept(&mut self, iter: usize, new_penalty: f64) -> bool {
        let increase = new_penalty - self.penalty;
        if increase <= 0.0 {
            return true;
        }
        if self.penalty <= 0. {
            if new_penalty > 0. || self.max_temp > 1. {
                return false;
            }
        }

        let temp = self.temp(iter);
        if temp <= 0. {
            return false;
        }
        let prob = (-increase / temp).exp();
        self.rng.gen::<f64>() < prob
    }

    fn temp(&self, iter: usize) -> f64 {
        let f = |x| self.temp_power(x, 2.5);

        if let Some(initial_temp) = self.initial_temp {
            let initial_temp_period = self.iteration / 10;
            if iter <= initial_temp_period {
                return initial_temp
                    - (initial_temp - self.max_temp) * (iter as f64) / initial_temp_period as f64;
            }
            return f((iter - initial_temp_period) * self.iteration
                / (self.iteration - initial_temp_period));
        }
        f(iter)
    }

    fn temp_linear(&self, iter: usize) -> f64 {
        let t = self.time(iter);
        (self.max_temp - self.min_temp) * t + self.min_temp
    }

    fn temp_power(&self, iter: usize, exp: f64) -> f64 {
        let t = self.time(iter);
        (self.max_temp - self.min_temp) * t.powf(exp) + self.min_temp
    }

    fn temp_exp(&self, iter: usize) -> f64 {
        let offset = self.min_temp - 1.;

        let start_m = self.max_temp - offset;
        let end_m = 1.0f64;

        let log_alpha = (end_m.log(E) - start_m.log(E)) / (self.iteration as f64);

        let log_t = start_m.log(E) + (iter as f64) * log_alpha;

        E.powf(log_t) + offset
    }

    // 1 -> 0
    fn time(&self, iter: usize) -> f64 {
        1. - iter as f64 / self.iteration as f64
    }

    fn flip_action(&self, v: Vector) -> Action {
        let mut res = vec![];
        for d in self.sim.iter() {
            res.push(self.board.applied(*d, &v));
        }
        res.sort();
        res.dedup();
        Action::Flip(res)
    }

    fn swap_action(&self, v1: Vector, v2: Vector) -> Option<Action> {
        let mut res = vec![];
        for d in self.sim.iter() {
            let a = self.board.applied(*d, &v1);
            let b = self.board.applied(*d, &v2);
            if a < b {
                res.push((a, b));
            } else {
                res.push((b, a));
            }
        }
        res.sort();
        res.dedup();
        for i in 0..res.len() {
            for j in 0..i {
                if res[i].0 == res[j].0
                    || res[i].1 == res[j].1
                    || res[i].0 == res[j].1
                    || res[i].1 == res[j].0
                {
                    return None;
                }
            }
        }
        Action::Swap(res).into()
    }

    fn zip_action(&self, v: Vector) -> Action {
        let mut res = vec![];
        for d in self.sim.iter() {
            res.push(self.board.applied(*d, &v));
        }
        res.sort();
        res.dedup();
        Action::Zip(res)
    }

    fn blink_action(&self, to_set: Vector, to_clear: Vector) -> Action {
        let mut blinks = vec![];
        for d in self.sim.iter() {
            let (a, c) = (
                self.board.applied(*d, &to_set),
                self.board.applied(*d, &to_clear),
            );
            blinks.push((a, c));
        }
        blinks.sort();
        blinks.dedup();

        let mut res = vec![];

        blinks.into_iter().for_each(|(a, c)| {
            res.push(c);
            res.push(a);
        });

        Action::Flip(res)
    }

    fn mass_flip_action(&self, origin: Vector, mask: usize, side: usize) -> Option<Action> {
        let rect = Rect::from_min_max(
            origin,
            origin + Vector::new(side as i32 - 1, side as i32 - 1),
        );
        let (min, max) = (rect.min_corner(), rect.max_corner());

        let exp_rect = Rect::from_min_max(min - Vector::new(1, 1), max + Vector::new(1, 1));

        for d in self.sim.iter().copied() {
            let (gmin, gmax) = (self.board.applied(d, &min), self.board.applied(d, &max));
            let rot_min = gmin.pairwise_min(&gmax);
            let rot_max = gmin.pairwise_max(&gmax);

            if d != D4::I
                && !exp_rect
                    .intersection(&Rect::from_min_max(rot_min, rot_max))
                    .is_empty()
            {
                return None;
            }
        }

        let mut res = vec![];

        for d in self.sim.iter().copied() {
            let (gmin, gmax) = (self.board.applied(d, &min), self.board.applied(d, &max));
            let rot_min = gmin.pairwise_min(&gmax);

            let mut m = 0;

            let mut mask = mask;
            while mask > 0 {
                let i = mask.trailing_zeros() as usize;
                let (x, y) = (i / side, i % side);

                let v = self
                    .board
                    .applied(d, &(origin + Vector::new(x as i32, y as i32)))
                    - rot_min;
                let j = v.x as usize * side + v.y as usize;
                m |= 1 << j;

                mask &= !(1 << i);
            }

            res.push((rot_min, m));
        }
        res.sort();

        let conn = if side == 2 {
            self.connections2.clone()
        } else {
            self.connections3.clone()
        };
        Action::MassFlip(conn, res, side).into()
    }

    fn print_state(&self, iter: usize) {
        let pc = self
            .penalizer
            .penalty_per_covering(&self.board, true, &self.sim);

        print_covering(&self.board, pc.get(0).map(|x| x.1.as_ref()).unwrap_or(&[]));

        info!(
            "seed: {} iter: {} ({:.1}%) penalty: {:.2}/{:.2} cov: {} temp: {:.2} iter/s: {:.1}",
            self.seed,
            iter,
            iter as f64 * 100.0 / self.iteration as f64,
            pc.get(0).map(|x| x.0).unwrap_or(0.0),
            self.penalty,
            pc.len(),
            self.temp(iter),
            iter as f64 / self.start_time.elapsed().as_secs_f64(),
        );
    }
}

fn print_covering(board: &Board, covering: &[E2]) {
    if covering.is_empty() {
        eprint!("{}", TightPoly::from(board.tree()));
        return;
    }

    debug_assert_ne!(covering.len(), 1);

    eprintln!("{} {}", board.height(), board.width());

    let h = board.height() + 10;
    let w = board.width() + 10;

    let mut min = Vector::new(i32::MAX, i32::MAX);
    let mut max = Vector::new(0, 0);

    let center = Vector::new(board.height() as i32 / 2, board.width() as i32 / 2);

    let mut map = vec![vec![".".normal(); w * 2]; h * 2];

    let off = Vector::new(h as i32, w as i32);

    for (i, m) in covering.iter().enumerate() {
        min = min.pairwise_min(&(m * &center + off));
        max = max.pairwise_max(&(m * &center + off));

        for v in board.tree().poly().iter() {
            let v = m * &v + off;
            debug_assert!(map[v.x as usize][v.y as usize] == ".".normal());
            map[v.x as usize][v.y as usize] = ["@", "#", "X", "*", "+"][i % 5].normal();
        }
    }
    for v in board.problem_cells() {
        let v = v + &off;
        map[v.x as usize][v.y as usize] = map[v.x as usize][v.y as usize].clone().red();

        min = min.pairwise_min(&v);
        max = max.pairwise_max(&v);
    }

    let mut nearest_corner: Option<(i32, Vector)> = None;
    'outer: for c in [
        Vector::new(0, 0),
        Vector::new(0, board.width() as i32 - 1),
        Vector::new(board.height() as i32 - 1, 0),
        Vector::new(board.height() as i32 - 1, board.width() as i32 - 1),
    ] {
        for m in covering.iter() {
            let mc = m * &c + off;

            let dx = if mc.x < min.x {
                min.x - mc.x
            } else if mc.x > max.x {
                mc.x - max.x
            } else {
                0
            };
            let dy = if mc.y < min.y {
                min.y - mc.y
            } else if mc.y > max.y {
                mc.y - max.y
            } else {
                0
            };
            let d = dx + dy;

            if d == 0 {
                nearest_corner = None;
                break 'outer;
            }

            if nearest_corner.is_none() || nearest_corner.as_ref().unwrap().0 > d {
                nearest_corner = Some((d, mc));
            }
        }
    }
    if let Some((_, c)) = nearest_corner {
        min = min.pairwise_min(&c);
        max = max.pairwise_max(&c);
    }

    for i in min.x..=max.x {
        for j in min.y..=max.y {
            eprint!("{}", map[i as usize][j as usize]);
        }
        eprintln!();
    }
}

#[derive(Clone)]
enum Action {
    Flip(Vec<Vector>),
    Swap(Vec<(Vector, Vector)>),
    Zip(Vec<Vector>),
    MassFlip(Arc<Connections>, Vec<(Vector, usize)>, usize),
}

impl Action {
    fn try_apply(&self, board: &mut Board, rev: bool) -> anyhow::Result<()> {
        match self {
            Self::Flip(cells) => {
                for i in 0..cells.len() {
                    let i2 = if rev { cells.len() - 1 - i } else { i };

                    let res = board.try_flip(cells[i2].x as usize, cells[i2].y as usize);
                    if res.is_err() {
                        for j in (0..i).rev() {
                            let j2 = if rev { cells.len() - 1 - j } else { j };
                            board
                                .try_flip(cells[j2].x as usize, cells[j2].y as usize)
                                .unwrap();
                        }
                        return res;
                    }
                }

                Ok(())
            }
            Self::Swap(vs) => {
                for i in 0..vs.len() {
                    let i2 = if rev { vs.len() - 1 - i } else { i };

                    let res = board.try_swap(vs[i2].0, vs[i2].1);
                    if res.is_err() {
                        for j in (0..i).rev() {
                            let j2 = if rev { vs.len() - 1 - j } else { j };

                            board
                                .try_swap(vs[j2].0, vs[j2].1)
                                .with_context(|| {
                                    anyhow!(
                                        "{}swap {:?} {:?} failed",
                                        TightPoly::from(board.tree()),
                                        vs[j2].0,
                                        vs[j2].1
                                    )
                                })
                                .unwrap();
                        }
                        return res;
                    }
                }
                Ok(())
            }
            Self::Zip(vs) => {
                for i in 0..vs.len() {
                    let i2 = if rev { vs.len() - 1 - i } else { i };

                    let res = board.try_zip(vs[i2]);
                    if res.is_err() {
                        for j in (0..i).rev() {
                            let j2 = if rev { vs.len() - 1 - j } else { j };

                            board.try_zip(vs[j2]).unwrap();
                        }
                        return res;
                    }
                }
                Ok(())
            }
            Self::MassFlip(conn, vs, side) => {
                for i in 0..vs.len() {
                    let i2 = if rev { vs.len() - 1 - i } else { i };

                    let res = board.try_mass_flip(conn, vs[i2].0, vs[i2].1, *side);
                    if res.is_err() {
                        for j in (0..i).rev() {
                            let j2 = if rev { vs.len() - 1 - j } else { j };

                            board
                                .try_mass_flip(conn, vs[j2].0, vs[j2].1, *side)
                                .unwrap();
                        }
                        return res;
                    }
                }
                Ok(())
            }
        }
    }

    fn undo(&self, board: &mut Board) {
        match self {
            Self::Flip(..) => self.try_apply(board, true).unwrap(),
            Self::Swap(..) => self.try_apply(board, true).unwrap(),
            Self::Zip(..) => self.try_apply(board, true).unwrap(),
            Self::MassFlip(..) => self.try_apply(board, true).unwrap(),
        }
    }
}

fn penalty(board: &Board, ppc: &Vec<(f64, Vec<E2>)>) -> f64 {
    let mut res = 0.0;

    let bb = board.bounding_box();
    if bb.area() <= 25 {
        res += INITIAL_TEMP;
    }
    if bb.height().min(bb.width()) <= (board.height().min(board.width()) / 5).min(7) {
        res += INITIAL_TEMP;
    }

    res += ppc.iter().map(|x| x.0).sum::<f64>();

    if res <= 0. {
        let sz = board.height() * board.width();
        let bb = board.bounding_box().area() as usize;

        let asymm = TightPoly::from(board.tree()).asymmetricity();

        res -= (sz - bb) as f64 // 1..sz
            + (sz - board.tree().poly().cell_count()) as f64 / sz as f64 // 1/sz..1
            + (1.0 - asymm) / sz as f64; // 0..1/sz
        assert!(res <= 0.);
    }

    res
}

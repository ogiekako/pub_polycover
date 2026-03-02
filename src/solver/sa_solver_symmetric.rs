use std::{
    collections::BTreeMap,
    f64::consts::E,
    ops::{Deref, Range},
    str::FromStr,
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};

use anyhow::{anyhow, Context};
use colored::*;
use log::{error, info};
use rand::{rngs::SmallRng, Rng, SeedableRng};
use rayon::prelude::*;

use crate::{
    check::check::{self, full_check},
    data::{
        board::Board, connection::Connections, d4::D4, outline::Outline, rect::Rect,
        tight_poly::TightPoly, tree_poly::TreePoly, vector::Vector,
    },
};

use super::{parameters::Parameters, penalizer::Penalizer};

static CONNECTIONS: [OnceLock<Mutex<BTreeMap<TightPoly, Arc<Connections>>>>; 2] =
    [OnceLock::new(), OnceLock::new()];

pub fn init(problem: &TightPoly) {
    connections(problem, 2);
    connections(problem, 3);
}

fn connections(problem: &TightPoly, side: usize) -> Arc<Connections> {
    let mut g = CONNECTIONS[side - 2]
        .get_or_init(|| Default::default())
        .lock()
        .unwrap();

    g.entry(problem.clone())
        .or_insert_with(|| Arc::new(Connections::new_with_problem(side, problem)))
        .clone()
}

pub fn solve(
    name: &str,
    problem: TightPoly,
    side: usize,
    num_iter: usize,
    verbose: bool,
    nproc: usize,
    max_temp: Option<f64>,
    initial_cand: Option<Vec<(Vector, TightPoly)>>,
    no_retry: bool,
    no_early_return: bool,
    always_include_other_coverings: bool,
    penalizer: Box<dyn Penalizer>,
    params: Option<(Parameters, f64)>,
    forbidden: usize,
    base_seed: Option<u64>,
) -> Result<Vec<TightPoly>, Vec<(f64, TightPoly, Parameters)>> {
    if initial_cand.is_none() {
        assert!(num_iter > 25_000);
    }

    let connections = [connections(&problem, 2), connections(&problem, 3)];

    if initial_cand.is_none() {
        let mut core = None;
        for c in if side % 2 == 0 {
            vec![
                r#"4 4
####
#..#
#..#
#..#
"#,
                r#"6 6
##..##
.####.
..#...
..#...
.####.
##..##
"#,
                r#"6 6
######
#....#
#....#
#....#
#....#
##..##
"#,
                r#"8 8
..####..
.##..##.
##....##
#......#
#......#
##....##
.##..##.
..#..#..
"#,
            ]
        } else {
            vec![
                "1 1\n#",
                r#"5 5
.###.
##.##
#...#
...##
..##.
"#,
            ]
        } {
            let c = TightPoly::from_str(c).unwrap();
            let s = 8 + c.height();
            let offset = Vector::new(
                (s / 2 - c.width() / 2) as i32,
                (s / 2 - c.height() / 2) as i32,
            );
            let mut params = Parameters::default();
            params.act_flip = 50;
            params.act_clear = 5;
            let Ok(mut solver) = Solver::new(
                problem.clone(),
                s,
                20_000,
                0,
                false,
                Arc::new(Mutex::new((f64::MAX, 0))),
                Some(INITIAL_TEMP),
                None,
                (offset, c.clone()).into(),
                false,
                false,
                connections.clone(),
                penalizer.clone(),
                (params, 1.0).into(),
                0,
            ) else {
                continue;
            };
            let res = solver.solve(true);
            match res {
                Ok(res) => {
                    if res.height() <= side && res.width() <= side {
                        return Ok(vec![res]);
                    }
                    core = Some(c);
                    break;
                }
                Err((penalty, tree)) => {
                    let poly = TightPoly::from(tree);
                    if *penalty >= INITIAL_TEMP || poly.height().min(poly.width()) <= c.height() + 2
                    {
                        continue;
                    } else {
                        core = Some(c);
                        break;
                    }
                }
            }
        }

        let core = core.unwrap();
        let offset = Vector::new(
            (side / 2 - core.width() / 2) as i32,
            (side / 2 - core.height() / 2) as i32,
        );

        return solve(
            name,
            problem,
            side,
            num_iter,
            verbose,
            nproc,
            max_temp,
            Some(vec![(offset, core); nproc]),
            no_retry,
            no_early_return,
            always_include_other_coverings,
            penalizer,
            params,
            forbidden,
            base_seed,
        );
    }

    if verbose {
        info!("Solving {} with side {} and iter {}", name, side, num_iter);
    }

    let mut seeds = vec![];

    {
        let seed = base_seed.unwrap_or(
            side as u64
                + num_iter as u64
                + initial_cand
                    .as_ref()
                    .map(|x| x.get(0).map(|x| x.1.height() as u64).unwrap_or(0))
                    .unwrap_or(0),
        );
        let mut seed_rng = SmallRng::seed_from_u64(seed);

        for _ in 0..nproc {
            seeds.push(seed_rng.gen::<u64>());
        }
    }

    let shared_min: Arc<Mutex<(f64, u64)>> = Arc::new(Mutex::new((f64::MAX, 0)));

    let initial_temp = match initial_cand.as_ref() {
        Some(cand) => {
            if cand[0].1.height() <= 8 {
                Some(INITIAL_TEMP)
            } else {
                None
            }
        }
        None => Some(INITIAL_TEMP),
    };

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
                initial_temp,
                max_temp,
                initial_cand
                    .as_ref()
                    .map(|x| x[seed as usize % x.len()].clone()),
                no_early_return,
                always_include_other_coverings,
                connections.clone(),
                penalizer.clone(),
                params.clone(),
                forbidden,
            )
            .map_err(|_| {
                (
                    f64::INFINITY,
                    TightPoly::from_str("0 0").unwrap(),
                    Parameters::default(),
                )
            })?;
            let params = solver.params.clone();
            let res = solver.solve(no_retry);
            if res.is_ok() {
                *shared_min.lock().unwrap() = (0.0, seed);
            }
            res.map_err(|x| (x.0, TightPoly::from(&x.1), params))
        })
        .collect::<Vec<_>>();

    if results.iter().any(|x| x.is_ok()) {
        Ok(results.into_iter().filter_map(|x| x.ok()).collect())
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
        Err(min_cands)
    }
}

struct Solver {
    seed: u64,

    board: Board,
    iteration: usize,
    rng: SmallRng,

    // penalty per covering
    ppc: Vec<(f64, Vector, Vector)>,
    penalty: f64,
    min_penalty: (f64, TreePoly),

    verbose: bool,

    shared_min: Arc<Mutex<(f64, u64)>>,
    winning: bool,

    retry_threshold: usize,
    no_improve_count: usize,

    initial_temp: Option<f64>,
    min_temp: f64,
    max_temp: f64,

    no_early_return: bool,
    always_include_other_coverings: bool,

    connections: [Arc<Connections>; 2],

    start_time: Instant,

    penalizer: Box<dyn Penalizer>,

    params: Parameters,
    forbidden_area: Vec<bool>,
    use_frontness_threshold: bool,

    cov2_check_depth: usize,
}

const INITIAL_RETRY_THRESHOLD: usize = 50_000;

const INITIAL_TEMP: f64 = 1_000_000.0;

impl Solver {
    fn new(
        problem: TightPoly,
        side: usize,
        iteration: usize,
        seed: u64,
        verbose: bool,
        shared_min: Arc<Mutex<(f64, u64)>>,
        initial_temp: Option<f64>,
        max_temp: Option<f64>,
        initial_cand: Option<(Vector, TightPoly)>,
        no_early_return: bool,
        always_include_other_coverings: bool,
        connections: [Arc<Connections>; 2],
        mut penalizer: Box<dyn Penalizer>,
        params: Option<(Parameters, f64)>,
        forbidden: usize,
    ) -> anyhow::Result<Self> {
        let mut rng = SmallRng::seed_from_u64(
            seed + {
                if let Some(c) = initial_cand.as_ref() {
                    c.1.cells().len() as u64
                } else {
                    0
                }
            },
        );

        let params = if let Some((params, prob)) = params {
            if rng.gen_range(0.0..1.0) < prob {
                params
            } else {
                Parameters::random(&mut rng)
            }
        } else {
            Parameters::random(&mut rng)
        };

        penalizer.init(params.clone());

        let mut board = Board::new_with_allowed_d4s(
            problem.clone(),
            side,
            side,
            vec![D4::I],
            initial_cand.clone(),
            params.initial_cov2_check_depth,
        )?;

        if initial_cand.is_none() {
            board.start_transaction();
            board.try_flip(side / 2, side / 2).unwrap();
            board.commit_transaction();
        }

        let mut ppc = vec![];
        penalizer.penalty_per_covering_sym(&board, &mut ppc);
        let penalty = penalty(&board, &ppc, params.run_full_check);

        let mut mint = params.min_temp;
        let mut maxt = max_temp.unwrap_or(params.max_temp);
        if mint > maxt {
            std::mem::swap(&mut mint, &mut maxt);
        }

        assert!(forbidden * 2 < side);
        let mut forbidden_area = vec![false; side * side];
        for d in [D4::I, D4::R1, D4::R2, D4::R3] {
            let outline = Outline::new(side, side);
            for x in 0..forbidden {
                for y in 0..forbidden {
                    let (x2, y2) = outline.cell(x, y).transform(d);
                    let i = x2 * side + y2;
                    forbidden_area[i] = true;
                }
            }
        }

        let cov2_check_depth = params.initial_cov2_check_depth;

        Ok(Self {
            seed,
            min_penalty: (penalty, board.tree().clone()),
            board,
            iteration,
            rng,
            ppc,
            penalty,
            shared_min,
            winning: false,
            verbose,
            retry_threshold: INITIAL_RETRY_THRESHOLD,
            no_improve_count: 0,
            initial_temp,
            min_temp: mint,
            max_temp: maxt,
            no_early_return,
            always_include_other_coverings,
            connections,
            start_time: Instant::now(),
            penalizer,
            params,
            forbidden_area,
            use_frontness_threshold: false,
            cov2_check_depth,
        })
    }

    fn solve(&mut self, no_retry: bool) -> Result<TightPoly, &(f64, TreePoly)> {
        let mut iter = 0;
        while iter < self.iteration {
            iter += 1;
            if (self.penalty <= INITIAL_FULL_CHECK_FAIL_PENALTY
                && iter % 1_000 == 0
                && self.shared_min.lock().unwrap().0 <= 0.0)
                || iter % 100_000 == 0
                || iter >= self.iteration
                || self.penalty <= 0.0
            {
                if self.penalty <= 0.0 && (iter == self.iteration || !self.no_early_return) {
                    if self.min_penalty.0 < self.penalty {
                        self.reset_board_to_min();
                    }

                    if self.verbose {
                        info!(
                            "Found solution with seed {} and params {:?}",
                            self.seed, self.params
                        );
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

                if iter > self.iteration / 2
                    && self.penalty >= INITIAL_TEMP
                    && self.min_penalty.0 > sm.0 * 100.0
                {
                    if self.verbose {
                        info!("Giving up: penalty {} is too large", self.penalty);
                    }
                    return Err(&self.min_penalty);
                }
            }

            let updated = self.step(iter);

            let use_tie = updated && self.rng.gen_range(0.0..1.0) < self.params.use_tie_prob;

            let strictly_improved = self.penalty < self.min_penalty.0;

            if strictly_improved || self.penalty == self.min_penalty.0 && use_tie {
                let mut ok = true;
                if !self.params.run_full_check && self.penalty <= 0.0 {
                    ok = full_check(self.board.problem(), &TightPoly::from(self.board.tree()))
                        .is_ok();
                    if !ok {
                        self.min_penalty.0 = self.penalty; // not to run full check again
                    }
                }

                let bb = self.board.bounding_box();
                if ok
                    && bb.height() >= self.board.height() / 2
                    && bb.width() >= self.board.width() / 2
                {
                    self.no_improve_count = 0;
                    self.retry_threshold = (self.retry_threshold / 4).max(INITIAL_RETRY_THRESHOLD);

                    self.min_penalty = (self.penalty, self.board.tree().clone());
                }
            }
            if !strictly_improved && !no_retry {
                self.no_improve_count += 1;
                if self.no_improve_count >= self.retry_threshold {
                    self.no_improve_count = 0;
                    iter -= self.retry_threshold;
                    self.retry_threshold *= 4;

                    self.reset_board_to_min();

                    if self.winning && self.verbose {
                        let _g = self.shared_min.lock().unwrap();
                        self.print_state(iter);
                        info!("Reset to best state");
                    }
                }
            }
        }

        self.reset_board_to_min();
        if self.winning && self.verbose {
            let _g = self.shared_min.lock().unwrap();
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

        let orig_ppc_len = self.ppc.len();

        self.penalizer
            .penalty_per_covering_sym(&self.board, &mut self.ppc);
        let mut new_penalty = penalty(
            &self.board,
            &self.ppc[orig_ppc_len..],
            self.params.run_full_check,
        );

        // if new_penalty == INITIAL_FULL_CHECK_FAIL_PENALTY {
        // {
        //     self.cov2_check_depth -= 1;
        //     self.increase_cov2_check_depth();
        //     let mut ppc = vec![];
        //     self.penalizer
        //         .penalty_per_covering_sym(&self.board, &mut ppc);
        //     assert!(ppc.is_empty());
        // }
        // }
        while new_penalty == INITIAL_FULL_CHECK_FAIL_PENALTY {
            self.increase_cov2_check_depth();
            {
                let _g = self.shared_min.lock().unwrap();
                self.print_state(iter);
            }

            self.penalizer
                .penalty_per_covering_sym(&self.board, &mut self.ppc);
            new_penalty = penalty(
                &self.board,
                &self.ppc[orig_ppc_len..],
                self.params.run_full_check,
            );
        }

        if self.accept(iter, new_penalty) {
            self.ppc.drain(..orig_ppc_len);
            self.penalty = new_penalty;
            return true;
        }

        self.ppc.truncate(orig_ppc_len);

        self.board.start_transaction();
        action.undo(&mut self.board);
        self.board.commit_transaction();

        debug_assert!({
            let mut ppc = vec![];
            self.penalizer
                .penalty_per_covering_sym(&self.board, &mut ppc);
            let diff = penalty(&self.board, &ppc, self.params.run_full_check) - self.penalty;

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
            self.use_frontness_threshold =
                self.rng.gen_range(0.0..1.0) < self.params.use_frontness_threshold_prob;

            let use_restriction = self.rng.gen_range(0.0..1.0) < self.params.use_restriction_prob;

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

            for _ in 0..bb.area() {
                if let Some(action) = self.random_action_inner(allowed_dists.clone()) {
                    return action;
                }
            }
        }
    }

    fn is_forbidden(&self, allowed_dists: &Option<Range<i32>>, p: Vector) -> bool {
        debug_assert!(p.x >= 0 && p.y >= 0);

        let side = self.board.width() as i32;
        let center = Vector::new(side / 2, side / 2);

        if p == center {
            return true;
        }
        if self.forbidden_area[(p.x * side + p.y) as usize] {
            return true;
        }
        if self.use_frontness_threshold {
            let frontness = self.board.board_analyzer_symmetric().frontness(p);
            if frontness >= self.params.frontness_threshold {
                return true;
            }
        }

        if let Some(allowed_dists) = allowed_dists.as_ref() {
            let d: Vector = p - center;
            let dist = d.x.abs() + d.y.abs();
            !allowed_dists.contains(&dist)
        } else {
            false
        }
    }

    fn random_action_inner(&mut self, allowed_dists: Option<Range<i32>>) -> Option<Action> {
        let side = self.board.width() as i32;
        let bb = self.board.bounding_box();
        let (min, max) = (bb.min_corner(), bb.max_corner());

        match self.rng.gen_range(0..1000) {
            // Flip a cell (0..300)
            num if (0..0 + self.params.act_flip).contains(&num)
                || (100..100 + self.params.act_clear).contains(&num)
                || (200..200 + self.params.act_set).contains(&num) =>
            {
                if let Some(value) =
                    self.random_flip_action(bb, min, max, side, num, &allowed_dists)
                {
                    return value;
                }
            }
            // Swap
            num if (300..300 + self.params.act_swap).contains(&num) => {
                if let Some(value) = self.random_swap_action(bb, min, max, &allowed_dists) {
                    return value;
                }
            }
            // Clear a cell to break a covering
            num if (400..400 + self.params.act_clear_to_break).contains(&num) => {
                if let Some(value) = self.random_clear_to_break_action(bb, &allowed_dists) {
                    return value;
                }
            }
            // Set a cell to break a covering
            num if (500..500 + self.params.act_set_to_break).contains(&num) => {
                if let Some(value) = self.random_set_to_break_action(min, max, &allowed_dists) {
                    return value;
                }
            }
            // Zip
            num if (600..600 + self.params.act_zip).contains(&num) => {
                if let Some(value) = self.random_zip_action(bb, min, max, &allowed_dists) {
                    return value;
                }
            }
            // Mass flip 2x2, 3x3
            num if (700..700 + self.params.act_2x2).contains(&num)
                || (800..800 + self.params.act_3x3).contains(&num) =>
            {
                if let Some(value) =
                    self.random_mass_flip_action(num, min, max, side, bb, &allowed_dists)
                {
                    return value;
                }
            }
            // Mass flip to break a covering
            num if (900..900 + self.params.act_2x2_to_break).contains(&num)
                || (950..950 + self.params.act_3x3_to_break).contains(&num) =>
            {
                if let Some(value) =
                    self.random_mass_flip_to_break_action(num, min, max, side, allowed_dists)
                {
                    return value;
                }
            }
            _ => (),
        };
        None
    }

    #[inline(never)]
    fn random_mass_flip_to_break_action(
        &mut self,
        num: usize,
        min: Vector,
        max: Vector,
        side: i32,
        allowed_dists: Option<Range<i32>>,
    ) -> Option<Option<Action>> {
        let s = if num < 950 { 2 } else { 3 };
        let mut cov2 = self.ppc.clone();
        if cov2.is_empty() {
            return Some(None);
        }
        for i in 0..cov2.len() - 1 {
            cov2[i + 1].0 += cov2[i].0;
        }
        let sum = cov2[cov2.len() - 1].0;
        let bb2 = {
            let (mut min, mut max) = (min, max);
            min = (min - Vector::new(s, s)).pairwise_max(&Vector::new(0, 0));
            max = (max + Vector::new(s, s)).pairwise_min(&Vector::new(side - 1, side - 1));
            Rect::from_min_max(min, max)
        };
        let (bb_min, bb_max) = (bb2.min_corner(), bb2.max_corner());

        for _ in 0..bb2.area().max(10) {
            let r = self.rng.gen_range(0.0..sum);

            let i = cov2
                .binary_search_by(|x| x.0.partial_cmp(&r).unwrap())
                .err()
                .unwrap_or(0);

            let (_, mut m1, mut m2) = &cov2[i];

            if self.rng.gen::<bool>() {
                std::mem::swap(&mut m1, &mut m2);
            }

            let r = (&m1 + &bb2).intersection(&(&m2 + &bb2));
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

            let mut offset = (&(-m1) + &area).min_corner();

            offset = offset.pairwise_max(&bb_min);
            offset = offset.pairwise_min(&(bb_max - Vector::new(s - 1, s - 1)));

            if self.is_forbidden(&allowed_dists, offset)
                || self.is_forbidden(&allowed_dists, offset + Vector::new(s - 1, s - 1))
            {
                continue;
            }

            let o1 = Rect::from_min_max(offset - Vector::new(1, 1), offset + Vector::new(s, s));
            let o2 = &(m1 - m2) + &o1;

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
                return Some(a.into());
            }
        }
        None
    }

    fn random_mass_flip_action(
        &mut self,
        num: usize,
        min: Vector,
        max: Vector,
        side: i32,
        bb: Rect,
        allowed_dists: &Option<Range<i32>>,
    ) -> Option<Option<Action>> {
        let s = if num < 800 { 2 } else { 3 };
        let (mut min, mut max) = (min, max);
        min -= Vector::new(s - 1, s - 1);
        min = min.pairwise_max(&Vector::new(0, 0));
        max = max.pairwise_min(&Vector::new(side / 2, side / 2));

        for _ in 0..bb.area().max(10) {
            let offset = Vector::new(
                self.rng.gen_range(min.x..=max.x),
                self.rng.gen_range(min.y..=max.y),
            );

            if self.is_forbidden(allowed_dists, offset) {
                continue;
            }

            if let Some(a) = self.make_mass_flip_on(offset, s as usize) {
                return Some(a.into());
            }
        }
        None
    }

    fn random_zip_action(
        &mut self,
        bb: Rect,
        min: Vector,
        max: Vector,
        allowed_dists: &Option<Range<i32>>,
    ) -> Option<Option<Action>> {
        for _ in 0..bb.area().max(10) {
            let v = Vector::new(
                self.rng.gen_range(min.x..=max.x),
                self.rng.gen_range(min.y..=max.y),
            );

            if self.is_forbidden(allowed_dists, v) {
                continue;
            }

            if !self.board.can_zip(v) {
                continue;
            }

            return Some(self.zip_action(v).into());
        }
        None
    }

    fn random_set_to_break_action(
        &mut self,
        min: Vector,
        max: Vector,
        allowed_dists: &Option<Range<i32>>,
    ) -> Option<Option<Action>> {
        let mut cov2 = self.ppc.clone();
        if cov2.is_empty() {
            return Some(None);
        }
        for i in 0..cov2.len() - 1 {
            cov2[i + 1].0 += cov2[i].0;
        }
        let sum = cov2[cov2.len() - 1].0;
        let bb2 = {
            let (mut min, mut max) = (min, max);
            min = (min - Vector::new(1, 1)).pairwise_max(&Vector::new(0, 0));
            max = (max + Vector::new(1, 1)).pairwise_min(&Vector::new(
                self.board.height() as i32 - 1,
                self.board.width() as i32 - 1,
            ));
            Rect::from_min_max(min, max)
        };

        for _ in 0..bb2.area().max(10) {
            let r = self.rng.gen_range(0.0..sum);

            let i = cov2
                .binary_search_by(|x| x.0.partial_cmp(&r).unwrap())
                .err()
                .unwrap_or(0);
            let cov = &cov2[i];

            let (m1, m2) = (cov.1, cov.2);
            let (m1, m2) = self.rng.gen::<bool>().then(|| (m1, m2)).unwrap_or((m2, m1));

            let r = (&m1 + &bb2).intersection(&(&m2 + &bb2));
            if r.is_empty() {
                continue;
            }
            let min = r.min_corner();
            let max = r.max_corner();
            let p = Vector::new(
                self.rng.gen_range(min.x..=max.x),
                self.rng.gen_range(min.y..=max.y),
            );
            let p1 = p - m1;
            let p2 = p - m2;

            let v = if p1 == p2 {
                p1
            } else if self.board.get(p1) && !self.board.get(p2) {
                p2
            } else if !self.board.get(p1) && self.board.get(p2) {
                p1
            } else {
                continue;
            };

            if self.is_forbidden(allowed_dists, v) {
                continue;
            }

            if self.board.can_set_v(v) {
                return Some(self.flip_action(v).into());
            }

            let to_remove = self.board.substitutables_for(v);
            if to_remove.is_empty() {
                continue;
            }
            let u = to_remove[self.rng.gen_range(0..to_remove.len())];

            if self.is_forbidden(allowed_dists, u) {
                continue;
            }

            if let Some(sw) = self.swap_action(v, u) {
                return Some(sw.into());
            }
        }
        None
    }

    fn random_clear_to_break_action(
        &mut self,
        bb: Rect,
        allowed_dists: &Option<Range<i32>>,
    ) -> Option<Option<Action>> {
        let mut cov2 = self.ppc.clone();
        if cov2.is_empty() {
            return Some(None);
        }
        for i in 0..cov2.len() - 1 {
            cov2[i + 1].0 += cov2[i].0;
        }
        let sum = cov2[cov2.len() - 1].0;

        for _ in 0..bb.area().max(10) {
            let r = self.rng.gen_range(0.0..sum);

            let i = cov2
                .binary_search_by(|x| x.0.partial_cmp(&r).unwrap())
                .err()
                .unwrap_or(0);
            let cov = &cov2[i];

            let (m1, m2) = (cov.1, cov.2);
            let (m1, m2) = self.rng.gen::<bool>().then(|| (m1, m2)).unwrap_or((m2, m1));

            let cells = self.board.problem_cells();
            let p = cells[self.rng.gen_range(0..cells.len())];

            let (a, b) = (p - m1, p - m2);

            let v = if self.board.board_analyzer_symmetric().get(a) {
                a
            } else {
                b
            };

            if self.is_forbidden(allowed_dists, v) {
                continue;
            }

            if self.board.can_clear_v(v) {
                return Some(self.flip_action(v).into());
            }

            let to_set = self.board.substitutables_for(v);
            if to_set.is_empty() {
                continue;
            }
            let u = to_set[self.rng.gen_range(0..to_set.len())];

            if let Some(sw) = self.swap_action(v, u) {
                return Some(sw.into());
            }
        }
        None
    }

    fn random_flip_action(
        &mut self,
        bb: Rect,
        min: Vector,
        max: Vector,
        side: i32,
        num: usize,
        allowed_dists: &Option<Range<i32>>,
    ) -> Option<Option<Action>> {
        for _ in 0..bb.area().max(10) {
            let v = Vector::new(
                self.rng
                    .gen_range((min.x - 1).max(0)..=(max.x + 1).min(side - 1)),
                self.rng
                    .gen_range((min.y - 1).max(0)..=(max.y + 1).min(side - 1)),
            );

            if (100..200).contains(&num) && !self.board.get(v) {
                continue;
            }
            if (200..300).contains(&num) && self.board.get(v) {
                continue;
            }

            if self.is_forbidden(allowed_dists, v) {
                continue;
            }

            if !self.board.can_flip(v) {
                continue;
            }

            return Some(self.flip_action(v).into());
        }
        None
    }

    fn random_swap_action(
        &mut self,
        bb: Rect,
        min: Vector,
        max: Vector,
        allowed_dists: &Option<Range<i32>>,
    ) -> Option<Option<Action>> {
        for _ in 0..bb.area().max(10) {
            let v = Vector::new(
                self.rng.gen_range(min.x..=max.x),
                self.rng.gen_range(min.y..=max.y),
            );
            if self.is_forbidden(allowed_dists, v) {
                continue;
            }

            let swap_cands = self.board.substitutables_for(v);
            if swap_cands.is_empty() {
                continue;
            }
            let u = swap_cands[self.rng.gen_range(0..swap_cands.len())];

            if self.board.rot180(&u) == u {
                continue;
            }

            if let Some(sw) = self.swap_action(v, u) {
                return Some(sw.into());
            }
        }
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

        let conn = &self.connections[side - 2];

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
        let f = |x| {
            if self.params.temp_exp {
                self.temp_exp(x)
            } else {
                self.temp_power(x, self.params.temp_power)
            }
        };

        if let Some(initial_temp) = self.initial_temp {
            let initial_temp_period = ((self.iteration as f64 * 0.02) as usize)
                .min(50_000)
                .max(25_000);
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
        for d in D4::all() {
            res.push(self.board.applied(d, &v));
        }
        res.sort();
        res.dedup();
        Action::Flip(res)
    }

    fn swap_action(&self, v1: Vector, v2: Vector) -> Option<Action> {
        let mut res = vec![];
        for d in D4::all() {
            let a = self.board.applied(d, &v1);
            let b = self.board.applied(d, &v2);
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
        for d in D4::all() {
            res.push(self.board.applied(d, &v));
        }
        res.sort();
        res.dedup();
        Action::Zip(res)
    }

    fn blink_action(&self, to_set: Vector, to_clear: Vector) -> Action {
        let mut blinks = vec![];
        for d in D4::all() {
            let (a, c) = (
                self.board.applied(d, &to_set),
                self.board.applied(d, &to_clear),
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

        let mut res = vec![];

        for d in D4::all() {
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

            let e = (rot_min, m);
            if !res.contains(&e) {
                if !res.is_empty() {
                    let dx = (e.0.x - res[0].0.x).abs() as usize;
                    let dy = (e.0.y - res[0].0.y).abs() as usize;
                    if dx <= side && dy <= side && !(dx == side && dy == side) {
                        return None;
                    }
                }
                res.push(e);
            }
        }

        let conn = self.connections[side - 2].clone();
        Action::MassFlip(conn, res, side).into()
    }

    fn print_state(&self, iter: usize) {
        let mut ppc = vec![];
        self.penalizer
            .penalty_per_covering_sym(&self.board, &mut ppc);

        let max = ppc
            .iter()
            .max_by(|x, y| x.0.partial_cmp(&y.0).unwrap())
            .map(|x| (x.1, x.2));

        print_covering(&self.board, &max);

        info!(
            "seed: {} iter: {} ({:.1}%) penalty: {:.1e}/{:.1e} cov: {} temp: {:.1e} iter/s: {:.1}",
            self.seed,
            iter,
            iter as f64 * 100.0 / self.iteration as f64,
            ppc.get(0).map(|x| x.0).unwrap_or(0.0),
            self.penalty,
            ppc.len(),
            self.temp(iter),
            iter as f64 / self.start_time.elapsed().as_secs_f64(),
        );
    }

    fn reset_board_to_min(&mut self) {
        let poly = TightPoly::from(&self.min_penalty.1);
        let offset = Vector::new(
            self.board.height() as i32 / 2 - poly.height() as i32 / 2,
            self.board.width() as i32 / 2 - poly.width() as i32 / 2,
        );

        self.board = Board::new_with_allowed_d4s(
            self.board.problem().clone(),
            self.board.height(),
            self.board.width(),
            vec![D4::I],
            Some((offset, poly.clone())),
            self.cov2_check_depth,
        )
        .unwrap();

        self.ppc.clear();
        self.penalizer
            .penalty_per_covering_sym(&self.board, &mut self.ppc);
        self.penalty = self.min_penalty.0;
    }

    fn increase_cov2_check_depth(&mut self) {
        self.cov2_check_depth += 1;

        let poly = TightPoly::from(self.board.tree());
        let offset = Vector::new(
            self.board.height() as i32 / 2 - poly.height() as i32 / 2,
            self.board.width() as i32 / 2 - poly.width() as i32 / 2,
        );

        info!("cov2 check depth <- {}", self.cov2_check_depth);

        self.board = Board::new_with_allowed_d4s(
            self.board.problem().clone(),
            self.board.height(),
            self.board.width(),
            vec![D4::I],
            Some((offset, poly)),
            self.cov2_check_depth,
        )
        .unwrap();
    }
}

fn print_covering(board: &Board, covering: &Option<(Vector, Vector)>) {
    let Some(covering) = covering else {
        eprintln!("{}", board.tree());
        return;
    };

    eprintln!("{} {}", board.height(), board.width());

    let h = board.height() + 10;
    let w = board.width() + 10;

    let mut min = Vector::new(i32::MAX, i32::MAX);
    let mut max = Vector::new(0, 0);

    let center = Vector::new(board.height() as i32 / 2, board.width() as i32 / 2);

    let mut map = vec![vec![".".normal(); w * 2]; h * 2];

    let off = Vector::new(h as i32, w as i32);

    for (i, m) in [covering.0, covering.1].iter().enumerate() {
        min = min.pairwise_min(&(m + &center + off));
        max = max.pairwise_max(&(m + &center + off));

        for v in board.tree().poly().iter() {
            let v = m + &v + off;
            debug_assert!(map[v.x as usize][v.y as usize] == ".".normal());
            map[v.x as usize][v.y as usize] = ["@", "#", "X", "*", "+"][i % 5].normal();
        }
    }
    for v in board.problem_cells() {
        let v = v + &off;
        map[v.x as usize][v.y as usize] = map[v.x as usize][v.y as usize].clone().red();

        assert_ne!(map[v.x as usize][v.y as usize].deref(), ".");

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
        for m in [covering.0, covering.1] {
            let mc = m + &c + off;

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

const INITIAL_FULL_CHECK_FAIL_PENALTY: f64 = 1e-18;

fn penalty(board: &Board, ppc: &[(f64, Vector, Vector)], run_full_check: bool) -> f64 {
    let mut res = 0.0;

    let bb = board.bounding_box();
    if bb.area() <= 49 {
        res += INITIAL_TEMP;
    }
    if bb.height().min(bb.width()) <= (board.height().min(board.width()) / 3).min(7) {
        res += INITIAL_TEMP;
    }
    res += board.cover2_queries_symmetric().deeply_insertable_count() as f64 * INITIAL_TEMP;

    if res > 0. {
        return res;
    }

    res += ppc.iter().map(|x| x.0).sum::<f64>();

    // Add penalty if bounding box is too small.
    let bb = board.bounding_box();
    let ratio = bb.area() as f64 / (board.height() * board.width()) as f64;
    res /= ratio;

    if res <= 0. && run_full_check {
        info!("running full check");

        if let Err(e) = check::full_check(board.problem(), &TightPoly::from(board.tree())) {
            info!("full check failed: {e}\n{}", board.tree());
            res += INITIAL_FULL_CHECK_FAIL_PENALTY;
        } else {
            info!("full check passed");
        }
    }

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

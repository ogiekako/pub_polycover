use std::{
    cell::{Ref, RefCell},
    collections::BTreeSet,
};

use super::{
    board_analyzer_symmetric::BoardAnalyzerSymmetric, connection::Connections,
    cover1_queries::Cover1Queries, cover2_queries::Cover2Queries,
    cover2_queries_symmetric::Cover2QueriesSymmetric, d4::D4, e2::E2, outline::Outline, rect::Rect,
    tight_poly::TightPoly, tree_poly::TreePoly, vector::Vector,
};

use anyhow::{bail, ensure, Result};

#[derive(Clone)]
pub struct Board {
    problem: TightPoly,
    problem_cells: Vec<Vector>,
    tree: TreePoly,

    d4s: Vec<D4>,
    base_movements: Vec<E2>,
    cover2_queries: Option<Cover2Queries>,
    cover2_queries_symmetric: Option<Cover2QueriesSymmetric>,

    cover1_queries: Cover1Queries,

    coverings2: RefCell<Option<Vec<(E2, E2)>>>,

    other_coverings: RefCell<Option<Vec<Vec<E2>>>>,

    bounding_box: RefCell<Option<Rect>>,

    full_mask: usize,

    check_tree: bool,
    check_cover1: bool,

    transaction: Option<Vec<Vector>>,

    board_analyzer_symmetric: BoardAnalyzerSymmetric,
}

impl Board {
    pub fn new(problem: TightPoly, max_height: usize, max_width: usize) -> Result<Self> {
        let default_cov2_check_depth = 5;
        Self::new_with_allowed_d4s(
            problem,
            max_height,
            max_width,
            D4::all().collect(),
            None,
            default_cov2_check_depth,
        )
    }

    pub fn new_with_initial_cand(
        problem: TightPoly,
        cand: &TightPoly,
        check_tree: bool,
    ) -> Result<Self> {
        let mut board = Self::new(problem, cand.height(), cand.width())?;
        board.check_tree = check_tree;
        let mut cells = cand.cells();
        while !cells.is_empty() {
            let prev_len = cells.len();
            cells.retain(|p| board.try_flip(p.x as usize, p.y as usize).is_err());
            ensure!(cells.len() < prev_len, "cand contains problem");
        }
        Ok(board)
    }

    pub fn new_with_allowed_d4s(
        problem: TightPoly,
        max_height: usize,
        max_width: usize,
        d4s: Vec<D4>,
        initial_cand: Option<(Vector, TightPoly)>,
        cov2_check_depth: usize,
    ) -> Result<Self> {
        let d4s = {
            let mut d4s = d4s.clone();
            d4s.sort();
            d4s
        };
        let problem_cells = problem.cells();

        let mut problem_idx = vec![vec![None; problem.width() as usize]; problem.height() as usize];
        for (i, &v) in problem_cells.iter().enumerate() {
            problem_idx[v.x as usize][v.y as usize] = Some(1 << i as u32);
        }

        let max_side = max_height.max(max_width) as usize;

        let cover1_queries = Cover1Queries::new(problem.clone(), max_side);

        let cover2_queries_symmetric = if d4s.len() == 1 && max_height == max_width {
            Some(Cover2QueriesSymmetric::new(
                problem.clone(),
                max_side,
                cov2_check_depth,
            ))
        } else {
            None
        };
        let cover2_queries = if cover2_queries_symmetric.is_none() {
            Cover2Queries::new(problem.clone(), max_side, max_side, d4s.len()).into()
        } else {
            None
        };

        let mut base_movements = vec![];
        for r in d4s.iter().copied() {
            let (dx, dy) = Outline::new(max_height, max_width).cell(0, 0).transform(r);
            base_movements.push(E2::new(r, (dx as i32, dy as i32).into()));
        }

        let full_mask = (1 << problem_cells.len()) - 1;

        let mut board = Self {
            problem_cells,
            problem,
            tree: TreePoly::new(max_height, max_width),
            d4s,
            base_movements,
            cover1_queries,
            cover2_queries,
            cover2_queries_symmetric,
            coverings2: None.into(),
            other_coverings: None.into(),
            bounding_box: None.into(),
            full_mask,
            check_tree: true,
            check_cover1: true,
            transaction: None,
            board_analyzer_symmetric: BoardAnalyzerSymmetric::new(max_height.max(max_width)),
        };

        if let Some(cand) = initial_cand {
            board.start_transaction();
            let mut cells = cand
                .1
                .cells()
                .iter()
                .map(|v| v + &cand.0)
                .collect::<Vec<_>>();
            while !cells.is_empty() {
                let prev_len = cells.len();
                cells.retain(|p| board.try_flip(p.x as usize, p.y as usize).is_err());
                ensure!(cells.len() < prev_len, "{:?}", cells);
            }

            if board.cover2_queries_symmetric.is_some() {
                board.check_tree = false;
                let outline = Outline::new(max_height, max_width);
                let mut added = BTreeSet::new();

                let cells = board.tree().poly().iter().collect::<Vec<_>>();
                for c in cells.iter() {
                    for d in D4::all() {
                        let (x, y) = outline.cell(c.x as usize, c.y as usize).transform(d);
                        let p = Vector::new(x as i32, y as i32);
                        if !board.get(p) && !added.contains(&p) {
                            added.insert(p);
                            board.try_flip(p.x as usize, p.y as usize)?;
                        }
                    }
                }
                board.check_tree = true;
            }

            board.commit_transaction();
        }
        Ok(board)
    }

    pub fn set_check_tree_for_testing(&mut self, check_tree: bool) {
        self.check_tree = check_tree;
    }

    pub fn use_cover2_queries_for_testing(&mut self) {
        self.cover2_queries_symmetric = None;

        let side = self.height().max(self.width());
        self.cover2_queries = Some(Cover2Queries::new(
            self.problem.clone(),
            side,
            side,
            self.d4s.len(),
        ));
    }

    pub fn set_check_cover1_for_testing(&mut self, check_cover1: bool) {
        self.check_cover1 = check_cover1;
    }

    pub fn problem(&self) -> &TightPoly {
        &self.problem
    }

    pub fn problem_cells(&self) -> &[Vector] {
        &self.problem_cells
    }

    pub fn rot180(&self, v: &Vector) -> Vector {
        Vector::new(
            self.height() as i32 - 1 - v.x,
            self.width() as i32 - 1 - v.y,
        )
    }

    pub fn applied(&self, d: D4, v: &Vector) -> Vector {
        assert!(v.x >= 0 && v.y >= 0);
        let l = Outline::new(self.height(), self.width());
        let (x, y) = l.cell(v.x as usize, v.y as usize).transform(d);
        Vector::new(x as i32, y as i32)
    }

    pub fn tree(&self) -> &TreePoly {
        &self.tree
    }

    pub fn height(&self) -> usize {
        self.tree.height()
    }

    pub fn width(&self) -> usize {
        self.tree.width()
    }

    pub fn can_set(&self, x: usize, y: usize) -> bool {
        self.tree.can_set(x, y)
    }

    pub fn can_set_v(&self, v: Vector) -> bool {
        v.x >= 0 && v.y >= 0 && self.tree.can_set(v.x as usize, v.y as usize)
    }

    pub fn can_clear(&self, x: usize, y: usize) -> bool {
        self.tree.can_clear(x, y)
    }

    pub fn can_clear_v(&self, v: Vector) -> bool {
        v.x >= 0 && v.y >= 0 && self.tree.can_clear(v.x as usize, v.y as usize)
    }

    fn in_transaction(&self) -> bool {
        self.transaction.is_some()
    }

    pub fn start_transaction(&mut self) {
        assert!(self.transaction.is_none());
        self.transaction = Some(vec![]);
    }

    pub fn commit_transaction(&mut self) {
        let mut ps = self.transaction.take().unwrap();
        ps.sort();
        let mut i = 0;

        let mut to_flip = vec![];
        while i < ps.len() {
            let mut j = i;
            while j < ps.len() && ps[i] == ps[j] {
                j += 1;
            }
            if (j - i) & 1 == 1 {
                to_flip.push(ps[i]);
            }
            i = j;
        }

        if let Some(c2qsym) = self.cover2_queries_symmetric.as_mut() {
            c2qsym.flip(&to_flip);

            for p in to_flip {
                self.board_analyzer_symmetric.flip(p);
            }
        } else {
            for p in to_flip {
                self.flip_cover2_queries(p.x as usize, p.y as usize);
            }
        }

        self.invalidate_cache();
    }

    pub fn transaction(&self) -> &[Vector] {
        self.transaction.as_deref().unwrap_or(&[])
    }

    pub fn try_flip(&mut self, x: usize, y: usize) -> Result<()> {
        ensure!(self.tree().poly().cell_count() > 1 || !self.get((x as i32, y as i32).into()));

        self.try_flip_no_empty_check(x, y)
    }

    pub fn try_flip_no_empty_check(&mut self, x: usize, y: usize) -> Result<()> {
        let p = Vector::new(x as i32, y as i32);
        self.cover1_queries.flip(p);

        if self.check_cover1 && !self.cover1_queries.coverings1().is_empty() {
            self.cover1_queries.flip(p);
            bail!("cover1 is not empty");
        }

        if self.check_tree {
            let res = self.tree.flip(x, y);

            if res.is_err() {
                self.cover1_queries.flip(p);
                return res;
            }
        }

        self.flip_cover2_queries(x, y);

        Ok(())
    }

    pub fn try_mass_flip(
        &mut self,
        conn: &Connections,
        origin: Vector,
        mask: usize,
        side: usize,
    ) -> Result<()> {
        debug_assert!(self.check_tree);

        if mask == 0 {
            return Ok(());
        }

        let mut orig_mask = 0usize;
        for x in 0..side {
            for y in 0..side {
                let v = origin + Vector::new(x as i32, y as i32);
                if self.get(v) {
                    orig_mask |= 1 << (x * side + y);
                }
            }
        }

        if mask == orig_mask && self.tree().poly().cell_count() == orig_mask.count_ones() as usize {
            bail!("Empties the board");
        }

        let mut to_flip = vec![];
        for x in 0..side {
            for y in 0..side {
                let i = x * side + y;
                if mask >> i & 1 != 0 {
                    let v = origin + Vector::new(x as i32, y as i32);
                    to_flip.push(v);
                }
            }
        }

        for v in to_flip.iter() {
            self.cover1_queries.flip(*v);
        }
        if self.check_cover1 && !self.cover1_queries.coverings1().is_empty() {
            for v in to_flip.iter() {
                self.cover1_queries.flip(*v);
            }
            bail!("cover1 is not empty");
        }

        if let Err(e) = self.tree.mass_flip(conn, origin, mask, side) {
            for v in to_flip.iter() {
                self.cover1_queries.flip(*v);
            }
            return Err(e);
        }

        for v in to_flip.iter() {
            self.flip_cover2_queries(v.x as usize, v.y as usize);
        }

        Ok(())
    }

    pub fn can_flip(&self, v: Vector) -> bool {
        debug_assert!(self.check_tree);

        if self.get(v) {
            self.can_clear_v(v)
        } else {
            self.can_set_v(v)
        }
    }

    fn flip_cover2_queries(&mut self, x: usize, y: usize) {
        let p = Vector::new(x as i32, y as i32);

        if let Some(t) = self.transaction.as_mut() {
            t.push(p);
            return;
        }

        self.invalidate_cache();

        if let Some(_c2qsym) = self.cover2_queries_symmetric.as_mut() {
            unimplemented!();
            // _c2qsym.flip(&[p]);
        } else {
            for (i, m) in self.base_movements.iter().enumerate() {
                self.cover2_queries.as_mut().unwrap().flip(i, m * &p);
            }
        }
    }

    pub fn other_coverings_coeff(&self) -> u64 {
        assert!(!self.in_transaction());

        if self.other_coverings().is_empty() {
            return 0;
        }
        let n = self.other_coverings()[0].len();

        let tight: TightPoly = (&self.tree).into();
        let rot90 = tight.roted90();
        let rot90_sym = tight == rot90;
        let rot_sym = tight == rot90.roted90();
        let flip_sym = tight == tight.flipped();

        let mut base = 1u64;
        if flip_sym {
            base *= 2;
        }
        if rot_sym {
            base *= 2;
        }
        if rot90_sym {
            base *= 2;
        }

        base /= 8 / self.d4s.len() as u64;

        // In case initial_cand is not symmetric.
        if base <= 0 {
            base = 1;
        }

        base.pow(n as u32)
    }

    pub fn try_swap(&mut self, v: Vector, u: Vector) -> Result<()> {
        self.cover1_queries.flip(v);
        self.cover1_queries.flip(u);
        if self.check_cover1 && !self.cover1_queries.coverings1().is_empty() {
            self.cover1_queries.flip(v);
            self.cover1_queries.flip(u);
            bail!("cover1 is not empty");
        }

        if self.check_tree {
            if let Err(e) = self.tree.try_swap(v, u) {
                self.cover1_queries.flip(v);
                self.cover1_queries.flip(u);
                return Err(e);
            }
        }

        self.flip_cover2_queries(v.x as usize, v.y as usize);
        self.flip_cover2_queries(u.x as usize, u.y as usize);

        Ok(())
    }

    pub fn try_zip(&mut self, v: Vector) -> Result<()> {
        debug_assert!(self.check_tree);

        let (a, b) = self.tree.try_zip(v)?;

        for u in [v, a, b] {
            self.cover1_queries.flip(u);
        }
        if self.check_cover1 && !self.cover1_queries.coverings1().is_empty() {
            for u in [v, a, b] {
                self.cover1_queries.flip(u);
            }
            self.tree.try_zip(v).unwrap();
            bail!("cover1 is not empty");
        }

        for u in [v, a, b] {
            self.flip_cover2_queries(u.x as usize, u.y as usize);
        }

        Ok(())
    }

    pub fn can_zip(&mut self, v: Vector) -> bool {
        debug_assert!(self.check_tree);

        if self.tree.try_zip(v).is_err() {
            return false;
        }
        self.tree.try_zip(v).unwrap();

        true
    }

    fn invalidate_cache(&mut self) {
        *self.coverings2.get_mut() = None;
        *self.other_coverings.get_mut() = None;
        *self.bounding_box.get_mut() = None;
    }

    pub fn substitutables_for(&self, v: Vector) -> Vec<Vector> {
        self.tree.substitutables_for(v)
    }

    #[inline(always)]
    pub fn get(&self, p: Vector) -> bool {
        self.tree.get_v(p)
    }

    fn maybe_update_other_coverings(&self) {
        if self.other_coverings.borrow().is_some() {
            return;
        }

        if self.coverings2().is_empty() {
            self.update_other_coverings(false);
        } else {
            self.update_other_coverings(true);
        }
    }

    fn update_other_coverings(&self, trivial_only: bool) {
        let mut oc = self.other_coverings.borrow_mut();
        let other_coverings = oc.insert(vec![]);

        let placements = self.bucket_placements(trivial_only);

        let mut dp: Vec<Vec<Vec<&(usize, Vector<isize>)>>> = placements
            .iter()
            .enumerate()
            .map(|(mask, p)| {
                if mask & 1 != 0 {
                    p.iter().map(|x| vec![x]).collect()
                } else {
                    vec![]
                }
            })
            .collect();
        let mut ndp: Vec<Vec<Vec<&(usize, Vector<isize>)>>> = vec![vec![]; self.full_mask + 1];

        for l in 2.. {
            let mut updated = false;

            for cover_only in [true, false] {
                if l <= 2 && cover_only {
                    continue;
                }
                if l > 2 && !cover_only && !ndp[self.full_mask].is_empty() {
                    continue;
                }

                for mask in 1..self.full_mask {
                    if dp[mask].is_empty() {
                        continue;
                    }

                    let need = if cover_only {
                        self.full_mask & !mask
                    } else {
                        1 << mask.trailing_ones()
                    };

                    let rest = self.full_mask & !(mask | need);

                    let mut subset = rest + (cover_only as usize);

                    while subset > 0 {
                        subset = (subset - 1) & rest;
                        let a = subset | need;
                        let b = mask | a;

                        for qs in dp[mask].iter() {
                            for p in placements[a].iter() {
                                if qs.iter().any(|(j, db)| {
                                    if let Some(c2qsym) = self.cover2_queries_symmetric.as_ref() {
                                        let d = db - &p.1;
                                        c2qsym.placeable(Vector::new(d.x as i32, d.y as i32))
                                            != Some(true)
                                    } else {
                                        if p.0 < *j {
                                            self.cover2_queries
                                                .as_ref()
                                                .unwrap()
                                                .overlap_unchecked(*j, p.0, db, &p.1)
                                        } else {
                                            self.cover2_queries
                                                .as_ref()
                                                .unwrap()
                                                .overlap_unchecked(p.0, *j, &p.1, db)
                                        }
                                    }
                                }) {
                                    continue;
                                }
                                ndp[b].push(qs.iter().copied().chain(std::iter::once(p)).collect());
                                updated = true;
                            }
                        }
                    }
                }
            }
            if !updated {
                return;
            }
            if l > 2 && !ndp[self.full_mask].is_empty() {
                for v in ndp[self.full_mask].iter() {
                    other_coverings.push(
                        v.into_iter()
                            .map(|(i, p)| {
                                let mut e = self.base_movements[*i];
                                e.d.x += p.x as i32;
                                e.d.y += p.y as i32;
                                e
                            })
                            .collect::<Vec<_>>(),
                    );
                }
                return;
            }

            std::mem::swap(&mut dp, &mut ndp);
            ndp.iter_mut().for_each(|v| v.clear());
        }
    }

    // List placements that can appear on covering with 3 or more.
    //
    // Returns (id, placement) in coverings2.
    fn bucket_placements(&self, trivial_only: bool) -> Vec<Vec<(usize, Vector<isize>)>> {
        let tight: TightPoly = (&self.tree).into();
        let rot90 = tight.roted90();
        let rot90_sym = tight == rot90;
        let rot_sym = tight == rot90.roted90();
        let flip_sym = tight == tight.flipped();

        let mut placements = vec![vec![]; self.full_mask + 1];

        let max_count = self.full_mask.count_ones() - 2;

        let bb = self.bounding_box();

        let min_bad = Vector::new(self.problem.height() as i32, self.problem.width() as i32)
            - bb.max_corner();
        let max_bad = -bb.min_corner();

        for (i, base_movement) in self.base_movements.iter().enumerate() {
            let r = self.d4s[i];
            if flip_sym {
                if self.d4s[0..i].contains(&r.flipped()) {
                    continue;
                }
            }
            if rot_sym {
                if self.d4s[0..i].contains(&r.roted(2)) {
                    continue;
                }
            }
            if rot90_sym
                && [r.roted(1), r.roted(3)]
                    .iter()
                    .any(|rr| self.d4s[0..i].contains(rr))
            {
                continue;
            }

            if !trivial_only {
                // TODO: Use cover2_queries_symmetric.
                for (mask, v) in self
                    .cover2_queries
                    .as_ref()
                    .unwrap()
                    .all_placements(i, max_count)
                {
                    placements[mask].push((i, Vector::new(v.x as isize, v.y as isize)));
                }
                continue;
            }

            let base_rect = base_movement * &bb;
            let bxr = base_rect.x_range();
            let byr = base_rect.y_range();

            // TODO: Use cover2_queries_symmetric.
            for (mask, v) in self
                .cover2_queries
                .as_ref()
                .unwrap()
                .all_placements(i, max_count)
            {
                if min_bad.x <= v.x && v.x <= max_bad.x && min_bad.y <= v.y && v.y <= max_bad.y {
                    continue;
                }
                let xr = bxr.start + &v.x..bxr.end + &v.x;
                let yr = byr.start + &v.y..byr.end + &v.y;

                if self
                    .problem_cells
                    .iter()
                    .enumerate()
                    .any(|(i, p)| xr.contains(&p.x) && yr.contains(&p.y) && (mask >> i & 1 == 0))
                {
                    continue;
                }

                placements[mask].push((i, Vector::new(v.x as isize, v.y as isize)));
            }
        }

        placements
    }

    fn update_coverings2(&self) {
        let mut c2 = self.coverings2.borrow_mut();
        let coverings2 = c2.insert(vec![]);

        if let Some(c2qsym) = self.cover2_queries_symmetric.as_ref() {
            for (da, db) in c2qsym.coverings2() {
                coverings2.push((E2::new(D4::I, *da), E2::new(D4::I, *db)));
            }
            return;
        }
        for i in 0..self.base_movements.len() {
            for j in 0..=i {
                for (da, db) in self.cover2_queries.as_ref().unwrap().coverings2(i, j) {
                    coverings2.push((da + &self.base_movements[i], db + &self.base_movements[j]));
                }
            }
        }
    }

    pub fn coverings1_for_testing(&self) -> &BTreeSet<Vector> {
        self.cover1_queries.coverings1()
    }

    pub fn coverings2(&self) -> Ref<'_, Vec<(E2, E2)>> {
        assert!(!self.in_transaction());

        if self.coverings2.borrow().is_none() {
            self.update_coverings2();
        }
        Ref::map(self.coverings2.borrow(), |c2| c2.as_ref().unwrap())
    }

    pub fn coverings2_symmetric(&self) -> impl Iterator<Item = &(Vector, Vector)> + '_ {
        self.cover2_queries_symmetric().coverings2()
    }

    pub fn other_coverings(&self) -> Ref<'_, Vec<Vec<E2>>> {
        self.maybe_update_other_coverings();
        Ref::map(self.other_coverings.borrow(), |oc| oc.as_ref().unwrap())
    }

    pub fn common_rect(&self, m1: &E2, m2: &E2) -> Rect {
        let rect = self.bounding_box();

        let r1 = m1 * &rect;
        let r2 = m2 * &rect;

        r1.intersection(&r2)
    }

    pub fn common_rect_symmetric(&self, m1: &Vector, m2: &Vector) -> Rect {
        let rect = self.bounding_box();

        let r1 = m1 + &rect;
        let r2 = m2 + &rect;

        r1.intersection(&r2)
    }

    pub(crate) fn bounding_box(&self) -> Rect {
        self.tree().poly().bounding_box()
    }

    #[inline(never)]
    pub(crate) fn unplug_len_per_direction8(&self, m1: &E2, m2: &E2) -> Vec<usize> {
        let (i, j, p, q) = {
            let (i, p) = self.index(m1);
            let (j, q) = self.index(m2);
            if j < i {
                (i, j, p, q)
            } else {
                (j, i, q, p)
            }
        };
        debug_assert!(!self.overlap(i, j, &p, &q));

        let mut res = vec![];

        for d in Vector::directions8() {
            let di = Vector::new(d.x as isize, d.y as isize);

            let mut consecutive = 0;

            let mut q = q;
            for k in 0.. {
                q += di;

                let placeable = if let Some(c2qsym) = self.cover2_queries_symmetric.as_ref() {
                    let d = p - q;
                    c2qsym.placeable(Vector::new(d.x as i32, d.y as i32)) == Some(true)
                } else {
                    !self.cover2_queries.as_ref().unwrap().overlap(i, j, &p, &q)
                };

                if placeable {
                    consecutive += 1;
                    if consecutive >= 2 {
                        res.push(k - 1);
                        break;
                    }
                } else {
                    consecutive = 0;
                }
                if k > 4 {
                    res.push(usize::MAX / 2);
                    break;
                }
            }
        }
        assert_eq!(res.len(), 8);
        res
    }

    #[inline(never)]
    pub(crate) fn unplug_len_per_direction8_symmetric(
        &self,
        m1: &Vector,
        m2: &Vector,
    ) -> impl Iterator<Item = usize> + '_ {
        let c2qsym = self.cover2_queries_symmetric.as_ref().unwrap();

        let d = m1 - m2;

        Vector::directions8().map(move |dir| {
            if d.x.abs() + d.y.abs() > (d.x + dir.x).abs() + (d.y + dir.y).abs() {
                return usize::MAX / 2;
            }

            let mut consecutive = 0;
            let mut df = d;
            for k in 0.. {
                df += dir;

                if c2qsym.placeable(df) == Some(true) {
                    consecutive += 1;
                    if consecutive >= 2 {
                        return k - 1;
                    }
                } else {
                    consecutive = 0;
                }
                if k > 4 {
                    return usize::MAX / 2;
                }
            }
            unreachable!();
        })
    }

    pub fn overlap_count(&self, m1: &E2, m2: &E2) -> usize {
        if let Some(c2qsym) = self.cover2_queries_symmetric.as_ref() {
            return c2qsym.overlap_count(m2.d - m1.d).unwrap_or(0);
        }

        let (i, j, p, q) = {
            let (i, p) = self.index(m1);
            let (j, q) = self.index(m2);
            if j < i {
                (i, j, p, q)
            } else {
                (j, i, q, p)
            }
        };
        self.cover2_queries
            .as_ref()
            .unwrap()
            .overlap_count(i, j, &p, &q)
    }

    pub fn overlap_count_symmetric(&self, m1: &Vector, m2: &Vector) -> usize {
        self.cover2_queries_symmetric
            .as_ref()
            .unwrap()
            .overlap_count(m2 - m1)
            .unwrap_or(0)
    }

    pub fn overlap(&self, i: usize, j: usize, p: &Vector<isize>, q: &Vector<isize>) -> bool {
        if let Some(c2qsym) = self.cover2_queries_symmetric.as_ref() {
            let d = p - q;
            return !c2qsym
                .placeable(Vector::new(d.x as i32, d.y as i32))
                .unwrap_or(true);
        }

        if i < j {
            self.cover2_queries.as_ref().unwrap().overlap(j, i, q, p)
        } else {
            self.cover2_queries.as_ref().unwrap().overlap(i, j, p, q)
        }
    }

    pub fn index(&self, m: &E2) -> (usize, Vector<isize>) {
        let i = self.base_movements.iter().position(|v| v.r == m.r).unwrap();
        let p = m.d - &self.base_movements[i].d;
        (i, Vector::new(p.x as isize, p.y as isize))
    }

    pub fn cover2_queries_symmetric(&self) -> &Cover2QueriesSymmetric {
        self.cover2_queries_symmetric.as_ref().unwrap()
    }

    pub fn board_analyzer_symmetric(&self) -> &BoardAnalyzerSymmetric {
        &self.board_analyzer_symmetric
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::data::{board::Board, tight_poly::TightPoly};

    #[test]
    fn test_coverings1_detected() {
        // ###
        // #..
        let problem = TightPoly::from_str("2 3\n###\n#..").unwrap();
        let mut board = Board::new(problem, 5, 5).unwrap();

        board.try_flip(2, 1).unwrap();
        board.try_flip(2, 2).unwrap();
        board.try_flip(2, 3).unwrap();
        assert!(board.try_flip(3, 1).is_err());
    }

    #[test]
    fn test_coverings2() {
        let problem = TightPoly::from_str(
            r#"5 3
    .#.
    .#.
    .##
    ###
    #..
    "#,
        )
        .unwrap();
        let mut board = Board::new(problem, 5, 5).unwrap();

        assert_eq!(*board.coverings2(), vec![]);

        board.try_flip(2, 1).unwrap();
        board.try_flip(2, 2).unwrap();
        board.try_flip(2, 3).unwrap();
        board.try_flip(2, 4).unwrap();

        assert_eq!(*board.coverings2(), vec![]);

        board.try_flip(2, 4).unwrap();
        board.try_flip(3, 1).unwrap();

        // ###
        // #..
        assert_eq!(board.coverings2().len(), 1);

        board.try_flip(2, 3).unwrap();

        // ##
        // #.
        assert_eq!(*board.coverings2(), vec![]);

        board.try_flip(2, 3).unwrap();
        board.try_flip(4, 1).unwrap();

        // ###
        // #..
        // #..
        assert_eq!(board.coverings2().len(), 4);

        board.try_flip(2, 3).unwrap();
        assert_eq!(board.coverings2().len(), 1);

        board.try_flip(2, 2).unwrap();
        board.try_flip(2, 1).unwrap();
        board.try_flip(3, 1).unwrap();

        assert_eq!(board.coverings2().len(), 0);
    }

    #[test]
    fn test_other_coverings() {
        let problem = TightPoly::from_str(
            r#"2 3
    ###
    ###
    "#,
        )
        .unwrap();
        let mut board = Board::new(problem, 5, 5).unwrap();

        assert_eq!(*board.other_coverings(), Vec::<Vec<_>>::new());

        board.try_flip(0, 0).unwrap();

        assert_eq!(board.other_coverings().len(), 1);

        board.try_flip(0, 1).unwrap();
        assert_eq!(board.other_coverings().len(), 3);

        board.try_flip(0, 2).unwrap();

        // 1**      11*      111      1**
        // *** x 2  *** x 4  *** x 4  1** x 10
        assert_eq!(board.other_coverings().len(), 20);
    }

    #[test]
    fn test_try_swap() {
        let problem = TightPoly::from_str(
            r#"3 3
    ###
    ###
    ###
    "#,
        )
        .unwrap();
        let mut board = Board::new(problem, 5, 5).unwrap();

        // #####
        // #....
        // #####
        // .#...

        board.try_flip(0, 0).unwrap();
        board.try_flip(0, 1).unwrap();
        board.try_flip(0, 2).unwrap();
        board.try_flip(0, 3).unwrap();
        board.try_flip(0, 4).unwrap();
        board.try_flip(1, 0).unwrap();
        board.try_flip(2, 0).unwrap();
        board.try_flip(2, 1).unwrap();
        board.try_flip(2, 2).unwrap();
        board.try_flip(2, 3).unwrap();
        board.try_flip(2, 4).unwrap();
        board.try_flip(3, 1).unwrap();

        // 1       1       1       1       3
        // .............................o......o..
        // ..........ooooo...........ooooo..ooooo.
        // #####...#####.o.#####o..#####.o.#####o.
        // #.ooooo.#.ooooo.#.ooooo.#.ooooo.#ooooo.
        // #####.o.#####o..#####.o.#####...#####..
        // .#ooooo..#.......#ooooo..#.......#.....
        // .....o.................................
        assert_eq!(board.coverings2().len(), 8 * 7);

        // #####
        // #...#
        // ##.##
        // .#...
        board.try_swap((2, 2).into(), (1, 4).into()).unwrap();

        // 1
        // #####o.
        // #.oo#oo
        // ##o##.o
        // .#ooooo
        assert_eq!(board.coverings2().len(), 8);

        board.try_swap((2, 2).into(), (1, 4).into()).unwrap();
        assert_eq!(board.coverings2().len(), 8 * 7);

        board.try_flip(3, 3).unwrap();

        board.try_swap((2, 2).into(), (1, 4).into()).unwrap();

        // #####
        // #...#
        // ##.##
        // .#.#.
        assert_eq!(board.coverings2().len(), 0);
        assert_eq!(board.other_coverings().len(), 0);

        board.try_swap((3, 2).into(), (0, 3).into()).unwrap();

        // ###.#
        // #...#
        // ##.##
        // .###.
        assert_eq!(board.coverings2().len(), 0);
    }
}

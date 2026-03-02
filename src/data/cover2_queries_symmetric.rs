use std::ops::Range;

use crate::data::outline::Outline;

use super::{
    d4::D4,
    e2::E2,
    tight_poly::TightPoly,
    u256::{U256, U512},
    vector::Vector,
};

const INF: i32 = 1 << 29;

#[derive(Clone)]
pub struct Cover2QueriesSymmetric {
    orig_n: usize,
    check_depth: usize,

    n: usize,
    m: usize,

    offset: Vector,
    center: Vector,

    shifts: Vec<E2>,
    mask_to_problem_ids: Vec<u8>,
    mask_to_problem0_ids: Vec<u8>,
    masks_union: usize,

    problem_cells: Vec<Vector>,

    // input
    flip_a: Vec<Vector>,
    set_a: Vec<Vector>,
    clear_mask: Vec<isize>,
    flip_na: Vec<Vector>,

    // A and N[A]
    a: Vec<bool>,
    a_masks: Vec<usize>,

    na_count: Vec<i32>,
    na_count_dup: Vec<i32>,
    na: Vec<U256>,

    // S(x) = min { y | (x,y) \in A } or INF
    smallest_a: Vec<i32>,
    prev_smallest_a: Vec<i32>,
    // L(x) = max { y | (x,y) \in N[A] } or -INF
    largest_na: Vec<i32>,

    // min { y | \exists x0 \in [x, x+m) (x0,y) \in A } or INF
    // = min { S(x), ..., S(x+m-1) }
    smallest_am: Vec<i32>,
    // max { y | \exists x0 \in [x, x+m) (x0,y) \in A } or -INF
    // = max { n-1-S(x), ..., n-1-S(x+m-1) }
    largest_am: Vec<i32>,

    // T(x) := max { L(x0) - S(x0+x) } = largest y s.t. N[A] \cap (A + (x,y)) \neq \emptyset
    prev_tangent: Vec<i32>,
    tangent: Vec<i32>,
    // x -> { L(x0) - S(x0+x) }
    tangent_bits: Vec<U512>,
    tangent_bits_count: Vec<usize>,

    delta_update_ranges: Vec<Option<Range<i32>>>,
    // (x,y) -> | A \cap (A + (x,y)) |
    // 0 <= x, y < n
    overlap: Vec<isize>,
    prev_placeable: Vec<isize>,
    placeable_pos: Vec<Vector>,

    // temporary
    flags: Vec<isize>,
    tag: isize,

    // (a,b) where (A+a) \xplus (A+b) covers the problem.
    coverings2: Vec<Vec<(Vector, Vector)>>,
}

impl Cover2QueriesSymmetric {
    pub fn new(problem: TightPoly, orig_n: usize, check_depth: usize) -> Self {
        let m = problem.height().max(problem.width());
        assert!(m <= 5);

        let offset = Vector::new(m as i32, m as i32);
        let n = m + orig_n + m;
        assert!(n <= U512::BITS as usize);

        let mut shifts = vec![];
        let mut mask_to_problem_ids = vec![0; 1 << m * m];
        let mut mask_to_problem0_ids = vec![0; 1 << m * m];

        let outline = Outline::new(problem.height(), problem.width());
        let problem_cells = problem.cells();
        let problem0 = problem_cells[0];

        let mut masks_union = 0;
        for r in D4::all() {
            let mut mask = 0;
            let mask0 = {
                let prob = problem.apply(r);
                for p in prob.cells().iter().copied() {
                    mask |= 1 << int(p, m);
                }
                let (x0, y0) = outline
                    .cell(problem0.x as usize, problem0.y as usize)
                    .transform(r);
                1 << x0 * m + y0
            };
            assert_eq!(mask & mask0, mask0);

            let zero = Vector::new(0, 0);
            let rp = r * Vector::new(problem.height() as i32 - 1, problem.width() as i32 - 1);
            let rc = r.inverse() * Vector::new(n as i32 - 1, n as i32 - 1);
            let dp = zero.pairwise_min(&rp);
            let dc = zero.pairwise_min(&rc);

            let ei = E2::new(r, -dp).inverse();
            shifts.push(E2::new(ei.r, ei.d + dc + offset));

            for i in 0..1 << m * m {
                if i & mask == mask {
                    mask_to_problem_ids[i] |= 1 << r as usize;
                }
                if i & mask0 == mask0 {
                    mask_to_problem0_ids[i] |= 1 << r as usize;
                }
            }

            masks_union |= mask;
        }

        Self {
            orig_n,
            check_depth,

            n,
            m,

            offset,
            center: Vector::new(orig_n as i32 / 2, orig_n as i32 / 2) + offset,

            shifts,

            problem_cells,
            mask_to_problem_ids,
            mask_to_problem0_ids,
            masks_union,

            flip_a: vec![],
            set_a: vec![],
            clear_mask: vec![0; n * n],
            flip_na: vec![],

            a: vec![false; n * n],
            a_masks: vec![0; n * n],

            na_count: vec![0; n * n],
            na_count_dup: vec![0; n * n],
            na: vec![0.into(); n],

            // largest_a = n - 1 - smallest_a
            smallest_a: vec![INF; n],
            prev_smallest_a: vec![INF; n],

            largest_na: vec![-INF; n],

            smallest_am: vec![INF; n],
            largest_am: vec![-INF; n],

            prev_tangent: vec![-INF; n],
            tangent: vec![-INF; n],
            tangent_bits: vec![0.into(); n],
            tangent_bits_count: vec![0; n * n],

            delta_update_ranges: vec![None; n],
            overlap: vec![0; n * n],
            prev_placeable: vec![0; n * n],
            placeable_pos: vec![],

            flags: vec![Default::default(); n * n],
            tag: 0,

            coverings2: vec![vec![]; n * n],
        }
    }

    pub fn placeable(&self, d: Vector) -> Option<bool> {
        self.overlap_count(d).map(|c| c == 0)
    }

    pub fn overlap_count(&self, d: Vector) -> Option<usize> {
        let mut dx = d.x.abs() as usize;
        let mut dy = d.y.abs() as usize;
        if dx > dy {
            std::mem::swap(&mut dx, &mut dy);
        }
        let from = (self.tangent[dx] - self.check_depth as i32 + 1).max(dx as i32) as usize;
        let to = self.tangent[dx] as usize;

        if dy >= to {
            0
        } else if dy < from {
            return None;
        } else {
            self.overlap[dx * self.n + dy] as usize
        }
        .into()
    }

    pub fn coverings2(&self) -> impl Iterator<Item = &(Vector, Vector)> + '_ {
        self.placeable_pos
            .iter()
            .flat_map(move |d| self.coverings2[d.x as usize * self.n + d.y as usize].iter())
    }

    pub fn deeply_insertable_count(&self) -> usize {
        let mut res = 0;
        for p in self.placeable_pos.iter() {
            if p.y + self.check_depth as i32 - 1 == self.tangent[p.x as usize] {
                res += 1;
            }
        }
        res
    }

    pub(crate) fn insert_depth_symmetric(&self, d: Vector) -> usize {
        let mut dx = d.x.abs();
        let mut dy = d.y.abs();
        if dx > dy {
            std::mem::swap(&mut dx, &mut dy);
        }

        (self.tangent[dx as usize] - dy) as usize
    }

    fn convert(&self, d: &Vector, s: &Vector, i: usize) -> (Vector, Vector) {
        let e = &self.shifts[i];

        let s2 = e * s;
        let sd2 = e.r * *d + &s2;

        (s2, sd2)
    }

    // to_flip must be symmetric.
    // the candidate must not directly cover the problem after flip.
    pub fn flip(&mut self, to_flip: &[Vector]) {
        if to_flip.is_empty() {
            return;
        }

        // Update a, a_masks, na, na_count, flip_a, flip_na
        self.update_a_na(to_flip);

        let clear_sign = self.tag;

        // Update smallest_a, largest_na, prev_tangent, tangent, tangent_bits, tangent_bits_count
        self.update_tangent();

        self.tag += 1;
        for d in self.placeable_pos.iter() {
            self.prev_placeable[int(*d, self.n)] = self.tag;
        }
        let sign = self.tag;

        // Update overlap
        self.update_overlap();

        // Update coverings2
        self.update_coverings2(sign, clear_sign);
    }

    #[inline(never)]
    fn update_a_na(&mut self, to_flip: &[Vector]) {
        assert!(to_flip.iter().all(|p| {
            (0..self.orig_n as i32).contains(&p.x) && (0..self.orig_n as i32).contains(&p.y)
        }));

        self.flip_a.clear();
        self.flip_a.extend_from_slice(to_flip);
        self.flip_a.sort();
        self.flip_a.iter_mut().for_each(|p| *p += self.offset);

        self.set_a.clear();
        self.flip_na.clear();

        self.tag += 1;
        for p in self.flip_a.iter().copied() {
            self.a[int(p, self.n)] ^= true;

            for dx in 0..self.m {
                for dy in 0..self.m {
                    let q = p - &Vector::new(dx as i32, dy as i32);

                    debug_assert!(
                        q.x >= 0 && q.y >= 0 && q.x < self.n as i32 && q.y < self.n as i32
                    );

                    self.a_masks[int(q, self.n)] ^= 1 << dx * self.m + dy;
                }
            }

            if self.a[int(p, self.n)] {
                self.set_a.push(p);
            } else {
                for d in self.problem_cells.iter() {
                    let q = p - d;
                    self.clear_mask[int(q, self.n)] = self.tag;
                }
            }

            if p.y < self.center.y {
                continue;
            }

            let inc = if self.a[int(p, self.n)] { 1 } else { -1 };
            for q in p.neighbors4().chain(std::iter::once(p)) {
                let c = &mut self.na_count_dup[int(q, self.n)];
                if *c == 0 || *c == -inc {
                    self.na[q.x as usize].flip((q.y - self.center.y + 1) as usize);
                }
                *c += inc;

                debug_assert!(*c >= 0);
            }
        }
        for p in &self.flip_a {
            if p.y < self.center.y {
                continue;
            }

            for q in p.neighbors4().chain(std::iter::once(*p)) {
                let i = int(q, self.n);
                let prev = self.na_count[i];
                let cur = self.na_count_dup[i];

                if (prev == 0) != (cur == 0) {
                    self.flip_na.push(q);
                }
                self.na_count[i] = cur;
            }
        }

        debug_assert_eq!(
            {
                let mut na = self.flip_na.clone();
                na.sort();
                na.dedup();
                na.len()
            },
            self.flip_na.len()
        );
    }

    #[inline(never)]
    fn update_tangent(&mut self) {
        self.prev_tangent.clear();
        self.prev_tangent.extend_from_slice(&self.tangent);

        self.prev_smallest_a.clear();
        self.prev_smallest_a.extend_from_slice(&self.smallest_a);

        let mut seen = U512::zero();

        let mut need_update_am = U512::zero();
        for p in &self.flip_a {
            if seen.get(p.x as usize) {
                continue;
            }
            seen.set(p.x as usize);
            let mut smallest_y = (0..self.n as i32)
                .find(|&y| self.a[int(Vector::new(p.x, y), self.n)])
                .unwrap_or(U512::BITS as i32);
            if smallest_y == U512::BITS as i32 {
                smallest_y = INF;
            }
            let v = &mut self.smallest_a[p.x as usize];
            if *v != smallest_y {
                for x in 0..p.x + 1 {
                    let x0 = p.x - x;
                    let prev = self.largest_na[x0 as usize] - *v;
                    let cur = self.largest_na[x0 as usize] - smallest_y;
                    let tan = &mut self.tangent[x as usize];

                    if prev >= 0 {
                        let p = (x * self.n as i32 + prev) as usize;
                        self.tangent_bits_count[p] -= 1;
                        if self.tangent_bits_count[p] == 0 {
                            self.tangent_bits[x as usize].clear(prev as usize);

                            if *tan == prev {
                                *tan = (U512::BITS - 1) as i32
                                    - self.tangent_bits[x as usize].leading_zeros() as i32;
                                if *tan < 0 {
                                    *tan = -INF;
                                }
                            }
                        }
                    }
                    if cur >= 0 {
                        let c = (x * self.n as i32 + cur) as usize;
                        self.tangent_bits_count[c] += 1;
                        if self.tangent_bits_count[c] == 1 {
                            self.tangent_bits[x as usize].set(cur as usize);

                            if *tan < cur {
                                *tan = cur;
                            }
                        }
                    }
                }
                *v = smallest_y;

                for i in 0..self.m {
                    need_update_am.set(p.x as usize - i);
                }
            }
        }

        for x in 0..self.n {
            if need_update_am.get(x) {
                self.smallest_am[x] = INF;
                self.largest_am[x] = -INF;
                for i in 0..self.m {
                    let x0 = x + i;
                    if self.smallest_a[x0] == INF {
                        continue;
                    }
                    self.smallest_am[x] = self.smallest_am[x].min(self.smallest_a[x0]);
                    self.largest_am[x] =
                        self.largest_am[x].max(self.n as i32 - 1 - self.smallest_a[x0]);
                }
            }
            self.prev_smallest_a[x] = self.prev_smallest_a[x].min(self.smallest_a[x]);
        }

        seen = U512::zero();

        for p in &self.flip_na {
            if seen.get(p.x as usize) {
                continue;
            }
            seen.set(p.x as usize);

            let mut largest_y =
                (U256::BITS - 1) as i32 - self.na[p.x as usize].leading_zeros() as i32;
            if largest_y < 0 {
                largest_y = -INF;
            } else {
                largest_y += self.center.y - 1;
            }
            let v = &mut self.largest_na[p.x as usize];
            if *v != largest_y {
                for x in 0..self.n as i32 - p.x {
                    let prev = -self.smallest_a[(x + p.x) as usize] + *v;
                    let cur = -self.smallest_a[(x + p.x) as usize] + largest_y;
                    let tan = &mut self.tangent[x as usize];

                    if prev >= 0 {
                        let p = (x * self.n as i32 + prev) as usize;
                        self.tangent_bits_count[p] -= 1;
                        if self.tangent_bits_count[p] == 0 {
                            self.tangent_bits[x as usize].clear(prev as usize);

                            if *tan == prev {
                                *tan = (U512::BITS - 1) as i32
                                    - self.tangent_bits[x as usize].leading_zeros() as i32;
                                if *tan < 0 {
                                    *tan = -INF;
                                }
                            }
                        }
                    }
                    if cur >= 0 {
                        let c = (x * self.n as i32 + cur) as usize;
                        self.tangent_bits_count[c] += 1;
                        if self.tangent_bits_count[c] == 1 {
                            self.tangent_bits[x as usize].set(cur as usize);

                            if *tan < cur {
                                *tan = cur;
                            }
                        }
                    }
                }
                *v = largest_y;
            }
        }
    }

    #[inline(never)]
    fn update_overlap(&mut self) {
        self.update_overlap_precompute();

        // Delta-update
        self.update_overlap_deltas();

        self.placeable_pos.retain(|pos| {
            let x = pos.x as usize;
            self.tangent[x] >= pos.y
                && (self.tangent[x] - self.check_depth as i32) < pos.y
                && self.overlap[x * self.n + pos.y as usize] == 0
        });
    }

    #[inline(never)]
    fn update_overlap_deltas(&mut self) {
        self.tag += 1;
        for p in self.flip_a.iter().copied() {
            self.flags[int(p, self.n)] = self.tag;
        }

        self.update_overlap_deltas_forward();
        self.update_overlap_deltas_backward();
    }

    #[inline(never)]
    fn update_overlap_deltas_backward(&mut self) {
        let mut cur_x = 0;
        let mut delta_update_start = INF;

        for p in self.flip_a.iter().copied() {
            while cur_x <= p.x as usize {
                if let Some(range) = self.delta_update_ranges[cur_x].as_ref() {
                    delta_update_start = delta_update_start.min(range.start);
                }
                cur_x += 1;
            }
            if p.y < delta_update_start {
                continue;
            }

            let i2 = int(p, self.n);

            let cur1 = self.a[i2];

            for (x2, s) in self.prev_smallest_a[0..p.x as usize + 1].iter().enumerate() {
                let x = p.x as usize - x2;

                let Some(ys) = self.delta_update_ranges[x].as_ref() else {
                    continue;
                };
                let y_end = (p.y + 1 - s).min(ys.end);

                for y in (ys.start..y_end).rev() {
                    let di = x * self.n + y as usize;
                    let i1 = i2 - di;

                    if self.a[i1] && self.flags[i1] != self.tag {
                        if cur1 {
                            self.overlap[di] += 1;
                        } else {
                            self.overlap[di] -= 1;

                            if self.overlap[di] == 0 {
                                let p = Vector::new(x as i32, y as i32);
                                if let Err(i) = self.placeable_pos.binary_search(&p) {
                                    self.placeable_pos.insert(i, p);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[inline(never)]
    fn update_overlap_deltas_forward(&mut self) {
        let mut cur_x: usize = 0;
        let mut delta_update_start = INF;
        for p in self.flip_a.iter().rev() {
            while cur_x + (p.x as usize) < self.n {
                if let Some(range) = self.delta_update_ranges[cur_x].as_ref() {
                    delta_update_start = delta_update_start.min(range.start);
                }
                cur_x += 1;
            }
            if self.n as i32 <= delta_update_start + p.y {
                continue;
            }

            let i1 = int(*p, self.n);

            let cur1 = self.a[i1];

            for (x, ys) in self.delta_update_ranges[0..(self.n - p.x as usize)]
                .iter()
                .enumerate()
            {
                let Some(ys) = ys.as_ref() else { continue };

                let x2 = p.x as usize + x;

                let s = &self.prev_smallest_a[x2];

                let y_start = ys.start.max(s - p.y);
                let y_end = (self.n as i32 - s - p.y).min(ys.end);

                for y in y_start..y_end {
                    let di = x * self.n + y as usize;
                    let i2 = i1 + di;

                    let v = &self.a[i2];

                    if cur1 && *v {
                        self.overlap[di] += 1;
                    } else if !cur1 && (*v != (self.flags[i2] == self.tag)) {
                        self.overlap[di] -= 1;

                        if self.overlap[di] == 0 {
                            let p = Vector::new(x as i32, y as i32);
                            if let Err(i) = self.placeable_pos.binary_search(&p) {
                                self.placeable_pos.insert(i, p);
                            }
                        }
                    }
                }
            }
        }
    }

    #[inline(never)]
    fn update_overlap_precompute(&mut self) {
        for x in 0..self.n {
            let cur_to = self.tangent[x] + 1;
            if cur_to <= 0 {
                self.delta_update_ranges[x] = None;
                continue;
            }
            let prev_to = self.prev_tangent[x] + 1;

            let cur_from = (cur_to - self.check_depth as i32).max(x as i32);
            let prev_from = (prev_to - self.check_depth as i32).max(x as i32);

            let mut range = prev_from.max(cur_from)..(prev_to.min(cur_to));
            range.end = range.end.max(range.start);

            for ys in [cur_from..range.start, range.end..cur_to] {
                for y in ys {
                    self.update_overlap_naive(x, y as usize);

                    if self.overlap[x * self.n + y as usize] == 0 {
                        let p = Vector::new(x as i32, y as i32);
                        if let Err(i) = self.placeable_pos.binary_search(&p) {
                            self.placeable_pos.insert(i, p);
                        }
                    }
                }
            }

            self.delta_update_ranges[x] = range.into();
        }
    }

    #[inline(never)]
    fn update_overlap_naive(&mut self, dx: usize, dy: usize) {
        let mut o = 0;

        let di = dx * self.n + dy;

        for x in dx..self.n {
            let x2 = x - dx;
            if self.smallest_a[x] == INF || self.smallest_a[x2] == INF {
                continue;
            }
            let to = self.n - self.smallest_a[x] as usize;
            let from = self.smallest_a[x2] as usize + dy;

            if to <= from {
                continue;
            }

            for i in x * self.n + from..x * self.n + to {
                if self.a[i] && self.a[i - di] {
                    o += 1;
                }
            }
        }

        self.overlap[di] = o;
    }

    #[inline(never)]
    fn update_coverings2(&mut self, placeable_sign: isize, clear_sign: isize) {
        for i in 0..self.placeable_pos.len() {
            if self.prev_placeable[int(self.placeable_pos[i], self.n)] == placeable_sign {
                self.update_cover2_delta(self.placeable_pos[i], clear_sign);
            } else {
                self.update_cover2_naive(self.placeable_pos[i]);
            }
        }
    }

    #[inline(never)]
    fn update_cover2_delta(&mut self, d: Vector, clear_sign: isize) {
        let di = int(d, self.n);

        if self.set_a.len() < self.flip_a.len() {
            let offi = int(self.offset, self.n) as i32;

            self.coverings2[di].retain(|(a, b)| unsafe {
                *self
                    .clear_mask
                    .get_unchecked((offi - b.x * self.n as i32 - b.y) as usize)
                    != clear_sign
                    && *self
                        .clear_mask
                        .get_unchecked((offi - a.x * self.n as i32 - a.y) as usize)
                        != clear_sign
            });
        }

        let retain_len = self.coverings2[di].len();
        let check_all = d.x == 0 || d.x == d.y;

        self.tag += 1;

        for p in self.set_a.iter() {
            if p.x + d.x > (self.n + self.m) as i32 {
                break;
            }
            if p.y + d.y > (self.n + self.m) as i32 {
                continue;
            }

            for x1 in p.x - self.m as i32 + 1..(p.x + 1).min(self.n as i32 - d.x) {
                for y1 in p.y.max(self.smallest_am[x1 as usize]) - self.m as i32 + 1
                    ..p.y.min(self.largest_am[(x1 + d.x) as usize] - d.y) + 1
                {
                    let si = (x1 * self.n as i32 + y1) as usize;

                    if self.flags[si] == self.tag {
                        continue;
                    }
                    self.flags[si] = self.tag;

                    let mask1 = self.a_masks[si] & self.masks_union;
                    let mask2 = self.a_masks[si + di] & self.masks_union;

                    debug_assert_eq!(mask1 & mask2, 0);

                    let ids =
                        self.mask_to_problem_ids[mask1 | mask2] & self.mask_to_problem0_ids[mask2];

                    if ids == 0 {
                        continue;
                    }

                    let sd = Vector::new(-x1 - d.x, -y1 - d.y);
                    for i in 0..8 {
                        if ids >> i & 1 == 0 {
                            continue;
                        }
                        let ab = self.convert(&d, &sd, i);

                        let l = if check_all {
                            self.coverings2[di].len()
                        } else {
                            retain_len
                        };

                        if !self.coverings2[di][0..l].contains(&ab) {
                            self.coverings2[di].push(ab);
                        }
                    }
                }
            }
        }

        for p in self.set_a.iter().rev() {
            if p.x <= d.x {
                break;
            }
            if p.y <= d.y {
                continue;
            }

            for x2 in (p.x - self.m as i32).max(d.x) + 1..p.x + 1 {
                for y2 in (p.y - self.m as i32)
                    .max(self.smallest_am[(x2 - d.x) as usize] - self.m as i32 + d.y)
                    + 1..p.y.min(self.largest_am[x2 as usize]) + 1
                {
                    let sdi = (x2 * self.n as i32 + y2) as usize;
                    let mask2 = self.a_masks[sdi] & self.masks_union;
                    let m = self.mask_to_problem0_ids[mask2];
                    if m == 0 {
                        continue;
                    }

                    debug_assert!(y2 <= self.largest_am[x2 as usize]);
                    debug_assert!(y2 - d.y > self.smallest_am[(x2 - d.x) as usize] - self.m as i32);

                    let si = sdi - di;

                    if self.flags[si] == self.tag {
                        continue;
                    }
                    self.flags[si] = self.tag;

                    let mask1 = self.a_masks[si] & self.masks_union;

                    debug_assert_eq!(mask1 & mask2, 0);

                    let ids = self.mask_to_problem_ids[mask1 | mask2] & m;

                    if ids == 0 {
                        continue;
                    }

                    let sd = Vector::new(-x2, -y2);
                    for i in 0..8 {
                        if ids >> i & 1 == 0 {
                            continue;
                        }

                        let ab = self.convert(&d, &sd, i);

                        let contains = if check_all {
                            self.coverings2[di].contains(&ab)
                        } else {
                            self.coverings2[di][0..retain_len].contains(&ab)
                        };

                        if !contains {
                            self.coverings2[di].push(ab);
                        }
                    }
                }
            }
        }
    }

    #[inline(never)]
    fn update_cover2_naive(&mut self, d: Vector) {
        debug_assert!(self.overlap[int(d, self.n)] == 0);
        debug_assert!(d.x <= d.y && d.y < self.n as i32);

        let di = int(d, self.n);
        self.coverings2[di].clear();

        let is_edge = d.x == 0 || d.x == d.y;

        for x0 in (d.x as i32 - self.m as i32 + 1).max(0) as usize..self.n - self.m + 1 {
            let mut to = -INF;
            let mut from = INF;
            for i in 0..self.m {
                to = to.max(self.n as i32 - self.smallest_a[x0 + i]);

                if x0 + i < d.x as usize {
                    continue;
                }
                let x1 = x0 + i - d.x as usize;
                from = from.min(self.smallest_a[x1] + d.y as i32 - self.m as i32 + 1);
            }
            if (from..to).is_empty() {
                continue;
            }
            for y0 in from as usize..to as usize {
                let mask1 = self.a_masks[x0 * self.n + y0] & self.masks_union;
                let mask2 = self.a_masks[(x0 - d.x as usize) * self.n + (y0 - d.y as usize)]
                    & self.masks_union;

                if self.mask_to_problem_ids[mask1] != 0 || self.mask_to_problem_ids[mask2] != 0 {
                    continue;
                }

                let m = mask1 | mask2;

                let ids = self.mask_to_problem_ids[m] & self.mask_to_problem0_ids[mask1];

                if ids == 0 {
                    continue;
                }

                for i in 0..8 {
                    if ids >> i & 1 == 0 {
                        continue;
                    }

                    let ab = self.convert(&d, &(&Vector::new(-(x0 as i32), -(y0 as i32))), i);

                    if !is_edge || !self.coverings2[di].contains(&ab) {
                        self.coverings2[di].push(ab);
                    }
                }
            }
        }
    }
}

fn int(p: Vector, n: usize) -> usize {
    debug_assert!(
        (0..n as i32).contains(&p.x) && (0..n as i32).contains(&p.y),
        "{:?}",
        p
    );
    (p.x as usize * n) + p.y as usize
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    use super::*;
    use crate::data::bit_poly::BitPoly;
    use crate::data::board::Board;
    use crate::data::d4::ALL_D4;
    use crate::data::outline::Outline;
    use crate::data::tight_poly::TightPoly;
    use crate::data::vector::Vector;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_update_tangent() {
        let problem = TightPoly::from_str("2 3\n###\n#.#").unwrap();
        let mut c2q = Cover2QueriesSymmetric::new(problem, 3, 8);
        c2q.flip(&[
            Vector::new(0, 1),
            Vector::new(1, 0),
            Vector::new(1, 1),
            Vector::new(1, 2),
            Vector::new(2, 1),
        ]);

        assert_eq!(c2q.tangent, vec![3, 2, 1, 0, -INF, -INF, -INF, -INF, -INF]);

        c2q.flip(&[
            Vector::new(0, 1),
            Vector::new(1, 0),
            Vector::new(1, 1),
            Vector::new(1, 2),
            Vector::new(2, 1),
        ]);
        assert_eq!(
            c2q.tangent,
            vec![-INF, -INF, -INF, -INF, -INF, -INF, -INF, -INF, -INF]
        );

        c2q.flip(&[Vector::new(1, 1)]);

        assert_eq!(
            c2q.tangent,
            vec![1, 0, -INF, -INF, -INF, -INF, -INF, -INF, -INF]
        );

        c2q.flip(&[
            Vector::new(0, 0),
            Vector::new(0, 2),
            Vector::new(2, 0),
            Vector::new(2, 2),
        ]);
        assert_eq!(c2q.tangent, vec![3, 2, 3, 2, -INF, -INF, -INF, -INF, -INF]);

        c2q.flip(&[
            Vector::new(0, 1),
            Vector::new(1, 0),
            Vector::new(1, 2),
            Vector::new(2, 1),
        ]);
        assert_eq!(c2q.tangent, vec![3, 3, 3, 2, -INF, -INF, -INF, -INF, -INF]);
    }

    #[test]
    fn test_placeable() {
        let problem = TightPoly::from_str("2 3\n###\n#.#").unwrap();
        let mut c2q = Cover2QueriesSymmetric::new(problem, 3, 8);
        c2q.flip(&[
            Vector::new(0, 1),
            Vector::new(1, 0),
            Vector::new(1, 1),
            Vector::new(1, 2),
            Vector::new(2, 1),
        ]);

        // .#.
        // ###
        // .#.

        assert!(!c2q.placeable(Vector::new(0, 0)).unwrap());
        assert!(!c2q.placeable(Vector::new(0, 1)).unwrap());
        assert!(!c2q.placeable(Vector::new(2, 0)).unwrap());
        assert!(!c2q.placeable(Vector::new(0, -2)).unwrap());
        assert!(!c2q.placeable(Vector::new(-1, 0)).unwrap());
        assert!(!c2q.placeable(Vector::new(-1, -1)).unwrap());

        assert!(c2q.placeable(Vector::new(1, 2)).unwrap());
        assert!(c2q.placeable(Vector::new(-2, 1)).unwrap());
        assert!(c2q.placeable(Vector::new(2, -1)).unwrap());
        assert!(c2q.placeable(Vector::new(-3, -0)).unwrap());
        assert!(c2q.placeable(Vector::new(-0, 3)).unwrap());
        assert!(c2q.placeable(Vector::new(-4, 0)).unwrap());

        c2q.flip(&[
            Vector::new(0, 0),
            Vector::new(0, 1),
            Vector::new(0, 2),
            Vector::new(1, 0),
            Vector::new(1, 1),
            Vector::new(1, 2),
            Vector::new(2, 0),
            Vector::new(2, 1),
            Vector::new(2, 2),
        ]);

        // #.#
        // ...
        // #.#

        assert!(!c2q.placeable(Vector::new(0, 0)).unwrap());
        assert!(!c2q.placeable(Vector::new(2, 0)).unwrap());
        assert!(!c2q.placeable(Vector::new(0, 2)).unwrap());
        assert!(!c2q.placeable(Vector::new(2, 2)).unwrap());

        assert!(c2q.placeable(Vector::new(0, 1)).unwrap());
        assert!(c2q.placeable(Vector::new(0, 3)).unwrap());
        assert!(c2q.placeable(Vector::new(1, 0)).unwrap());
        assert!(c2q.placeable(Vector::new(3, 0)).unwrap());

        assert!(c2q.placeable(Vector::new(-2, 1)).unwrap());
        assert!(c2q.placeable(Vector::new(1, 2)).unwrap());
        assert!(c2q.placeable(Vector::new(-3, -0)).unwrap());
        assert!(c2q.placeable(Vector::new(-0, 3)).unwrap());
        assert!(c2q.placeable(Vector::new(-2, 3)).unwrap());
        assert!(c2q.placeable(Vector::new(-4, 0)).unwrap());
    }

    #[test]
    fn test_update_coverings2() {
        let problem = TightPoly::from_str("2 3\n###\n#.#").unwrap();
        let mut c2q = Cover2QueriesSymmetric::new(problem, 5, 8);

        assert_eq!(c2q.m, 3);

        c2q.flip(&[
            Vector::new(1, 2),
            Vector::new(2, 1),
            Vector::new(2, 2),
            Vector::new(2, 3),
            Vector::new(3, 2),
        ]);

        // .....
        // ..#..
        // .###.
        // ..#..
        // .....

        assert_eq!(
            c2q.coverings2().copied().collect::<Vec<_>>(),
            vec![
                (Vector::new(-2, -2), Vector::new(-1, 0)),
                (Vector::new(-1, -2), Vector::new(-2, 0)),
            ]
        );

        c2q.flip(&[Vector::new(2, 2)]);

        // .....
        // ..#..
        // .#.#.
        // ..#..
        // .....

        assert_eq!(
            c2q.coverings2().copied().collect::<Vec<_>>(),
            vec![(Vector::new(-2, -1), Vector::new(-1, -1))]
        );
    }

    #[test]
    fn test_update_coverings2_small() {
        let problem = TightPoly::from_str("2 3\n###\n#.#").unwrap();
        let mut c2q = Cover2QueriesSymmetric::new(problem, 3, 8);

        c2q.flip(&[
            Vector::new(0, 1),
            Vector::new(1, 0),
            Vector::new(1, 1),
            Vector::new(1, 2),
            Vector::new(2, 1),
        ]);

        // .#.
        // ###
        // .#.

        assert_eq!(
            c2q.coverings2().copied().collect::<Vec<_>>(),
            vec![
                (Vector::new(-1, -1), Vector::new(0, 1)),
                (Vector::new(0, -1), Vector::new(-1, 1)),
            ]
        );
    }

    #[test]
    fn test_update_coverings2_random() {
        let mut rng = SmallRng::seed_from_u64(0);

        let side = 9;
        let mut seen = vec![-1i32; side * side];
        let mut tag = 0;

        let outline = Outline::new(side, side);

        for prob in ["2 3\n###\n#.#", "3 3\n.##\n##.\n.#."] {
            let problem = TightPoly::from_str(prob).unwrap();
            let mut c2q = Cover2QueriesSymmetric::new(problem.clone(), side, 10);
            let mut board =
                Board::new_with_allowed_d4s(problem.clone(), side, side, vec![D4::I], None, 5)
                    .unwrap();
            board.set_check_tree_for_testing(false);
            board.set_check_cover1_for_testing(false);
            board.use_cover2_queries_for_testing();

            let mut poly = BitPoly::new(side, side);
            for _ in 0..5000 {
                tag += 1;

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

                        if seen[x * side + y] == tag {
                            continue;
                        }
                        to_flip.push(Vector::new(x as i32, y as i32));
                        seen[x * side + y] = tag;
                    }
                }

                c2q.flip(&to_flip);
                for p in &to_flip {
                    board.try_flip(p.x as usize, p.y as usize).unwrap();
                    poly.flip(p);
                }

                if !board.coverings1_for_testing().is_empty() {
                    c2q.flip(&to_flip);
                    for p in to_flip.iter().rev() {
                        board.try_flip(p.x as usize, p.y as usize).unwrap();
                        poly.flip(p);
                    }
                }

                let mut got = c2q.coverings2().copied().collect::<Vec<_>>();
                let mut want = board
                    .coverings2()
                    .iter()
                    .map(|(m1, m2)| (m1.d, m2.d))
                    .collect::<Vec<_>>();

                want.retain(|(d1, d2)| c2q.placeable(d2 - d1).is_some());

                got.sort();
                want.sort();

                assert_eq!(got, want, "problem: {}{}{:?}", problem, poly, to_flip);
            }
        }
    }

    #[test]
    fn test_update_coverings2_random_big() {
        let mut rng = SmallRng::seed_from_u64(0);

        for (side, iter) in [(260, 100), (16, 1000)] {
            let mut seen = vec![-1i32; side * side];
            let mut tag = 0;

            let outline = Outline::new(side, side);

            for prob in ["2 3\n###\n#.#", "3 3\n###\n#..\n#..."] {
                let problem = TightPoly::from_str(prob).unwrap();
                let mut c2q = Cover2QueriesSymmetric::new(problem.clone(), side, 5);
                let mut board =
                    Board::new_with_allowed_d4s(problem.clone(), side, side, vec![D4::I], None, 5)
                        .unwrap();
                board.set_check_tree_for_testing(false);
                board.set_check_cover1_for_testing(false);
                board.use_cover2_queries_for_testing();

                let mut poly = BitPoly::new(side, side);
                for i in 0..iter {
                    tag += 1;

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

                            if seen[x * side + y] == tag {
                                continue;
                            }
                            to_flip.push(Vector::new(x as i32, y as i32));
                            seen[x * side + y] = tag;
                        }
                    }

                    c2q.flip(&to_flip);
                    for p in &to_flip {
                        board.try_flip(p.x as usize, p.y as usize).unwrap();
                        poly.flip(p);
                    }

                    if !board.coverings1_for_testing().is_empty() {
                        c2q.flip(&to_flip);
                        for p in to_flip.iter().rev() {
                            board.try_flip(p.x as usize, p.y as usize).unwrap();
                            poly.flip(p);
                        }
                    }

                    if i % 10 == 0 {
                        let mut got = c2q.coverings2().copied().collect::<Vec<_>>();
                        let mut want = board
                            .coverings2()
                            .iter()
                            .map(|(m1, m2)| (m1.d, m2.d))
                            .collect::<Vec<_>>();

                        want.retain(|(d1, d2)| c2q.placeable(d2 - d1).is_some());

                        got.sort();
                        want.sort();

                        assert_eq!(got, want, "problem: {}{}{:?}", problem, poly, to_flip);
                    }
                }
            }
        }
    }
}

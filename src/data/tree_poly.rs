use std::{
    collections::{BTreeSet, VecDeque},
    fmt::{Display, Formatter},
    str::FromStr,
    sync::Arc,
};

use anyhow::{bail, ensure, Context, Result};

use crate::data::{DX, DY};

use super::{
    bit_poly::BitPoly, connection::Connections, tight_poly::TightPoly, u256::U512, vector::Vector,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TreePoly {
    width: usize,
    poly: BitPoly,
}

impl Display for TreePoly {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{} {}", self.height(), self.width())?;
        let mut map = vec![vec!['.'; self.width()]; self.height()];
        for x in 0..self.height() {
            for y in 0..self.width() {
                if self.get(x, y) {
                    map[x][y] = '#';
                }
            }
        }
        for row in map {
            writeln!(f, "{}", row.iter().collect::<String>())?;
        }
        Ok(())
    }
}

impl From<&TreePoly> for TightPoly {
    fn from(value: &TreePoly) -> Self {
        let mut rows: Vec<U512> = value
            .poly()
            .rows
            .iter()
            .skip_while(|&row| row.is_zero())
            .take_while(|&row| !row.is_zero())
            .copied()
            .collect();

        if rows.is_empty() {
            return TightPoly::new(rows).unwrap();
        }

        let shift = value
            .poly()
            .rows
            .iter()
            .map(|&row| row.trailing_zeros())
            .min()
            .unwrap();

        rows.iter_mut().for_each(|x| *x >>= shift as usize);

        TightPoly::new(rows).unwrap()
    }
}

impl From<TreePoly> for TightPoly {
    fn from(value: TreePoly) -> Self {
        Self::from(&value)
    }
}

impl TreePoly {
    pub fn new(height: usize, width: usize) -> Self {
        Self {
            width,
            poly: BitPoly::new(height, width),
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.poly().height()
    }

    pub fn poly(&self) -> &BitPoly {
        &self.poly
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        self.poly.get(x, y)
    }

    pub fn set_leaf(&mut self, x: usize, y: usize) -> Result<()> {
        self.check_can_set(x, y)?;

        self.poly.flip(&Vector::new(x as i32, y as i32));

        Ok(())
    }

    pub fn set_leaf_v(&mut self, v: Vector) -> Result<()> {
        ensure!(v.x >= 0 && v.y >= 0);
        self.set_leaf(v.x as usize, v.y as usize)
    }

    fn check_can_set(&self, x: usize, y: usize) -> Result<Option<Vector>> {
        ensure!(x < self.height() && y < self.width());
        ensure!(!self.get(x, y));

        let mut adjacent = None;

        let p = Vector::new(x as i32, y as i32);
        for (d, np) in p.neighbors4().enumerate() {
            if self.get_v(np) {
                ensure!(adjacent.is_none());
                adjacent = np.into();

                for e in 0..2 {
                    let nnx = x as i32 + DX[(d + e + 1) & 3] + DX[(d + e + 2) & 3];
                    let nny = y as i32 + DY[(d + e + 1) & 3] + DY[(d + e + 2) & 3];
                    ensure!(!self.get_v(Vector::new(nnx, nny)));
                }
            }
        }
        ensure!(adjacent.is_some() || self.poly.is_empty());
        Ok(adjacent)
    }

    pub fn can_set(&self, x: usize, y: usize) -> bool {
        self.check_can_set(x, y).is_ok()
    }

    pub fn clear_leaf(&mut self, x: usize, y: usize) -> Result<()> {
        self.check_can_clear(x, y)?;
        self.poly.flip(&Vector::new(x as i32, y as i32));

        Ok(())
    }

    fn check_can_clear(&self, x: usize, y: usize) -> Result<Option<Vector>> {
        ensure!(self.get(x, y));

        let mut count = 0;
        let mut adjacent = None;

        let p = Vector::new(x as i32, y as i32);
        for np in p.neighbors4() {
            if self.get_v(np) {
                count += 1;
                adjacent = np.into();
            }
        }
        ensure!(count == 1 || self.poly.cell_count() == 1 && count == 0);
        Ok(adjacent)
    }

    pub fn can_clear(&self, x: usize, y: usize) -> bool {
        self.check_can_clear(x, y).is_ok()
    }

    pub fn flip(&mut self, x: usize, y: usize) -> Result<()> {
        if self.get(x, y) {
            self.clear_leaf(x, y)
        } else {
            self.set_leaf(x, y)
        }
    }

    #[inline(always)]
    pub(crate) fn get_v(&self, v: Vector) -> bool {
        self.poly.get_v(v)
    }

    // ..x..     ..x..
    // .#.#.     .###.
    // x###x <-> x.#.x
    // xx.xx     xx.xx
    pub fn try_zip(&mut self, v: Vector) -> Result<(Vector, Vector)> {
        let mut to_clear: Vec<Vector> = vec![];
        let mut to_set: Vec<Vector> = vec![];

        let res;

        if !self.get_v(v) {
            let mut empty = None;
            for u in v.neighbors4() {
                if !self.get_v(u) {
                    ensure!(empty.is_none());
                    empty = Some(u);
                }
            }
            ensure!(empty.is_some());
            let empty = empty.unwrap();

            to_set.push(v);
            for c in [1, 3] {
                let d = v - empty;
                let e = d.roted90(c);

                let u = v + d + e;
                ensure!(self.get_v(u));
                ensure!(!self.get_v(u + d));
                ensure!(!self.get_v(u + e));
                ensure!(!self.get_v(u + d + e));

                to_clear.push(u);
            }
            res = (to_clear[0], to_clear[1]);
        } else {
            let mut empty = None;
            for u in v.neighbors4() {
                if !self.get_v(u) {
                    ensure!(empty.is_none());
                    empty = Some(u);
                }
            }
            ensure!(empty.is_some());
            let empty = empty.unwrap();

            to_clear.push(v);
            for c in [1, 3] {
                let d = v - empty;
                let e = d.roted90(c);

                let u = v + d + e;
                ensure!(!self.get_v(u));
                ensure!(!self.get_v(u + d));
                ensure!(!self.get_v(u + e));
                ensure!(!self.get_v(u + d + e));

                to_set.push(u);
            }
            res = (to_set[0], to_set[1]);
        }

        // Update rows
        to_clear.iter().chain(to_set.iter()).for_each(|v| {
            self.poly.flip(v);
        });

        Ok(res)
    }

    fn substitutables_for_alive(&self, v: Vector) -> Vec<Vector> {
        assert!(self.get_v(v));

        let neighbors: Vec<_> = v.neighbors4().filter(|&u| self.get_v(u)).collect();
        if neighbors.len() != 2 {
            return vec![];
        }
        let (u1, u2) = (neighbors[0], neighbors[1]);
        if u1.x != u2.x && u1.y != u2.y {
            let d1 = u1 - v;
            let d2 = u2 - v;

            let x = v + d1 + d2;
            debug_assert!(!self.get_v(x));

            if self.get_v(x + d1) || self.get_v(x + d2) || self.get_v(x + d1 + d2) {
                return vec![];
            }
            return vec![x];
        }

        let mut cands = vec![BTreeSet::<Vector>::new(); 2];

        let first_step: Vec<_> = [u1, u2]
            .into_iter()
            .map(|u| Step::new(v + (u - v).roted90(1), v))
            .collect();
        let last_step: Vec<_> = [u1, u2]
            .into_iter()
            .map(|u| Step::new(v + (u - v).roted90(3), v))
            .collect();

        debug_assert!(self.is_valid_step(&first_step[0]) && self.is_valid_step(&last_step[0]));
        debug_assert!(self.is_valid_step(&first_step[1]) && self.is_valid_step(&last_step[1]));

        let mut step: Vec<_> = first_step
            .iter()
            .map(|x| x.nexts().find(|x| self.is_valid_step(&x.0)).unwrap().0)
            .collect();
        while step[0] != last_step[0] && step[1] != last_step[1] {
            for i in 0..2 {
                let n = step[i].lhs + (step[i].lhs - step[i].rhs);
                if self.get_v(n) && n != v {
                    if !cands[i].contains(&step[i].lhs) {
                        cands[i].insert(step[i].lhs);
                    } else {
                        cands[i].remove(&step[i].lhs);
                    }
                }
                step[i] = step[i]
                    .nexts()
                    .find(|x| self.is_valid_step(&x.0))
                    .unwrap()
                    .0;
            }
        }

        let i = if step[0] == last_step[0] { 0 } else { 1 };

        cands[i].iter().copied().collect()
    }

    pub fn substitutables_for(&self, v: Vector) -> Vec<Vector> {
        if self.get_v(v) {
            return self.substitutables_for_alive(v);
        }
        let neighbors: Vec<_> = v.neighbors4().filter(|&u| self.get_v(u)).collect();
        if neighbors.len() < 2 {
            return vec![];
        }
        if neighbors.len() == 3 {
            for u in neighbors {
                if self.get_v(v + (v - u)) {
                    continue;
                }
                if self.get_v(u + (u - v)) {
                    return vec![];
                }
                return vec![u];
            }
            unreachable!();
        }
        let (u1, u2) = (neighbors[0], neighbors[1]);
        if u1.x != u2.x && u1.y != u2.y {
            let d = Vector::new(u1.x + u2.x - v.x * 2, u1.y + u2.y - v.y * 2);
            if self.get_v(v - d) {
                return vec![];
            }
            debug_assert!(self.get_v(v + d), "{}", TightPoly::from(self));
            let mut res = vec![];
            if self.neighbors_count(v + d) == 2 {
                res.push(v + d);
            }
            if self.neighbors_count(u1) == 1 {
                res.push(u1);
            }
            if self.neighbors_count(u2) == 1 {
                res.push(u2);
            }
            return res;
        }

        let mut walk = {
            let mut walk1 = (vec![Step::new(v, u1)], 0);
            let mut walk2 = (vec![Step::new(v, u2)], 0);

            debug_assert!(self.is_valid_step(&walk1.0[0]));
            debug_assert!(self.is_valid_step(&walk2.0[0]));

            'outer: loop {
                if walk1.0[walk1.0.len() - 1] == walk2.0[0] && walk1.1 == 2 {
                    break 'outer walk1.0;
                } else if walk2.0[walk2.0.len() - 1] == walk1.0[0] && walk2.1 == 2 {
                    break 'outer walk2.0;
                }

                for (walk, rot) in [&mut walk1, &mut walk2] {
                    let step = &walk[walk.len() - 1];
                    for ns in step.nexts() {
                        if self.is_valid_step(&ns.0) {
                            walk.push(ns.0);
                            *rot += ns.1;
                            break;
                        }
                    }
                }
            }
        };

        walk.pop().unwrap();
        walk.remove(0);
        walk.sort_by_key(|step| step.rhs);

        let mut res = vec![];
        for i in 0..walk.len() {
            if i > 0 && walk[i - 1].rhs == walk[i].rhs {
                continue;
            }
            if i + 1 < walk.len() && walk[i + 1].rhs == walk[i].rhs {
                continue;
            }
            let u = walk[i].rhs + (walk[i].rhs - walk[i].lhs);
            if u == v || self.get_v(u) {
                continue;
            }
            res.push(walk[i].rhs);
        }
        res
    }

    fn is_valid_step(&self, step: &Step) -> bool {
        !self.get_v(step.lhs) && self.get_v(step.rhs)
    }

    fn neighbors_count(&self, v: Vector) -> usize {
        v.neighbors4().filter(|&u| self.get_v(u)).count()
    }

    // swap a living cell and a dead cell to make a new tree.
    pub fn try_swap(&mut self, v: Vector, u: Vector) -> Result<()> {
        let (to_clear, to_set) = if self.get_v(v) { (v, u) } else { (u, v) };
        ensure!(self.get_v(to_clear));
        ensure!(!self.get_v(to_set));
        ensure!(self.substitutables_for(to_set).contains(&to_clear));

        // Update rows
        self.poly.flip(&to_clear);
        self.poly.flip(&to_set);
        Ok(())
    }

    pub fn mask_2d(&self, origin: Vector, side: usize) -> usize {
        let mut mask = 0;
        for x in 0..side {
            for y in 0..side {
                let v = origin + Vector::new(x as i32, y as i32);
                if self.get_v(v) {
                    mask |= 1 << (x * side + y);
                }
            }
        }
        mask
    }

    pub(crate) fn mass_flip(
        &mut self,
        conn: &Connections,
        origin: Vector,
        mask: usize,
        side: usize,
    ) -> Result<()> {
        if mask == 0 {
            return Ok(());
        }

        let orig_mask_2d = self.mask_2d(origin - Vector::new(1, 1), side + 2);

        for x in 0..side {
            for y in 0..side {
                let i = x * side + y;
                if (mask >> i) & 1 != 0 {
                    let v = origin + Vector::new(x as i32, y as i32);
                    self.poly.flip(&v);
                }
            }
        }

        let cur_mask_2d = self.mask_2d(origin - Vector::new(1, 1), side + 2);

        if !Arc::ptr_eq(
            &conn.candidates(orig_mask_2d),
            &conn.candidates(cur_mask_2d),
        ) {
            for x in 0..side {
                for y in 0..side {
                    let i = x * side + y;
                    if (mask >> i) & 1 != 0 {
                        let v = origin + Vector::new(x as i32, y as i32);
                        self.poly.flip(&v);
                    }
                }
            }
            bail!("surroundings changed");
        }

        let mut to_clear = vec![];
        let mut to_set = vec![];
        for x in 0..side {
            for y in 0..side {
                let i = x * side + y;
                let v = origin + Vector::new(x as i32, y as i32);

                if mask >> i & 1 == 0 {
                    continue;
                }

                if self.get_v(v) {
                    to_set.push(v);
                } else {
                    to_clear.push(v);
                }
            }
        }

        Ok(())
    }

    pub(crate) fn is_leaf(&self, p: Vector) -> bool {
        if !self.get_v(p) {
            return false;
        }
        let mut c = 0;
        for q in p.neighbors4() {
            if self.get_v(q) {
                c += 1;
            }
        }
        c <= 1
    }

    fn leaves_slow(&self) -> Vec<Vector> {
        let mut res = vec![];
        for x in 0..self.height() {
            for y in 0..self.width() {
                if self.is_leaf(Vector::new(x as i32, y as i32)) {
                    res.push(Vector::new(x as i32, y as i32));
                }
            }
        }
        res
    }
}

impl FromStr for TreePoly {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut ss = s.split_ascii_whitespace();
        let height: usize = ss.next().context("height")?.parse()?;
        let width: usize = ss.next().context("width")?.parse()?;

        let map: Vec<_> = ss
            .map(|s| s.chars().map(|c| c == '#').collect::<Vec<_>>())
            .collect();

        ensure!(map.len() == height as usize);
        ensure!(map[0].len() == width as usize);

        let mut root: Option<Vector> = None;
        'outer: for x in 0..height {
            for y in 0..width {
                if map[x as usize][y as usize] {
                    root = Some(Vector::new(x as i32, y as i32));
                    break 'outer;
                }
            }
        }

        let mut tree = Self::new(height, width);

        if root.is_none() {
            return Ok(tree);
        }
        let root = root.unwrap();
        tree.set_leaf_v(root)?;

        let mut cells = VecDeque::new();
        cells.push_back(root);
        while let Some(cell) = cells.pop_front() {
            for v in cell.neighbors4() {
                if !tree.get_v(v)
                    && 0 <= v.x
                    && v.x < height as i32
                    && 0 <= v.y
                    && v.y < width as i32
                    && map[v.x as usize][v.y as usize]
                {
                    tree.set_leaf_v(v)?;
                    cells.push_back(v);
                }
            }
        }

        Ok(tree)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Step {
    // Empty
    lhs: Vector,
    // Wall
    rhs: Vector,
}

impl Step {
    fn new(lhs: Vector, rhs: Vector) -> Self {
        Self { lhs, rhs }
    }

    fn nexts(&self) -> impl Iterator<Item = (Self, /* rot90 */ i8)> {
        let d = (self.rhs - self.lhs).roted90(1);
        let v = self.rhs + d;
        let u = self.lhs + d;
        [
            (Self::new(v, self.rhs), -1),
            (Self::new(u, v), 0),
            (Self::new(self.lhs, u), 1),
        ]
        .into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::TreePoly;
    use crate::data::{tight_poly::TightPoly, vector::Vector};
    use std::str::FromStr;

    #[test]
    fn test_into() {
        let mut tree = TreePoly::new(5, 5);
        tree.set_leaf(2, 1).unwrap();
        tree.set_leaf(2, 2).unwrap();
        tree.set_leaf(3, 1).unwrap();
        tree.set_leaf(4, 1).unwrap();
        tree.set_leaf(4, 2).unwrap();

        let tight = TightPoly::from_str(
            r#"
3 2
##
#.
##
"#,
        )
        .unwrap();

        assert_eq!(TightPoly::from(tree), tight);
    }

    #[test]
    fn test_set() {
        let mut tree = TreePoly::new(5, 5);

        for (x, y, ok) in [
            (2, 2, true),
            (3, 3, false),
            (2, 3, true),
            (3, 2, true),
            (3, 3, false),
            (2, 4, true),
            (4, 2, true),
            (4, 2, false),
            (4, 3, true),
            (3, 4, false),
        ] {
            assert_eq!(tree.set_leaf(x, y).is_ok(), ok, "{x} {y}");
        }
    }

    #[test]
    fn test_clear() {
        let mut tree = TreePoly::new(5, 5);

        assert!(!tree.can_clear(0, 0));
        tree.set_leaf(0, 0).unwrap();
        assert_eq!(
            tree.leaves_slow().iter().copied().collect::<Vec<_>>(),
            [Vector::new(0, 0)]
        );
        tree.clear_leaf(0, 0).unwrap();
        assert_eq!(tree.leaves_slow().len(), 0);

        // #####
        // ..#..
        // #.###
        // #.#..
        // #####
        tree.set_leaf(0, 0).unwrap();
        tree.set_leaf(0, 1).unwrap();
        tree.set_leaf(0, 2).unwrap();
        tree.set_leaf(0, 3).unwrap();
        tree.set_leaf(0, 4).unwrap();
        tree.set_leaf(1, 2).unwrap();
        tree.set_leaf(2, 2).unwrap();
        tree.set_leaf(2, 3).unwrap();
        tree.set_leaf(2, 4).unwrap();
        tree.set_leaf(3, 2).unwrap();
        tree.set_leaf(4, 2).unwrap();
        tree.set_leaf(4, 3).unwrap();
        tree.set_leaf(4, 4).unwrap();
        tree.set_leaf(4, 1).unwrap();
        tree.set_leaf(4, 0).unwrap();
        tree.set_leaf(3, 0).unwrap();
        tree.set_leaf(2, 0).unwrap();

        let mut leaves: Vec<_> = tree.leaves_slow().iter().copied().collect();
        leaves.sort();
        assert_eq!(
            leaves,
            vec![
                Vector::new(0, 0),
                Vector::new(0, 4),
                Vector::new(2, 0),
                Vector::new(2, 4),
                Vector::new(4, 4),
            ]
        );

        for (x, y, ok) in [
            (0, 1, false),
            (0, 0, true),
            (0, 1, true),
            (0, 4, true),
            (2, 0, true),
            (2, 2, false),
        ] {
            assert_eq!(tree.clear_leaf(x, y).is_ok(), ok);
        }

        leaves = tree.leaves_slow().iter().copied().collect();
        leaves.sort();
        assert_eq!(
            leaves,
            vec![
                Vector::new(0, 3),
                Vector::new(2, 4),
                Vector::new(3, 0),
                Vector::new(4, 4),
            ]
        );

        let mut cells = tree.poly.iter().collect::<Vec<Vector>>();
        cells.sort();
        assert_eq!(
            cells,
            vec![
                Vector::new(0, 2),
                Vector::new(0, 3),
                Vector::new(1, 2),
                Vector::new(2, 2),
                Vector::new(2, 3),
                Vector::new(2, 4),
                Vector::new(3, 0),
                Vector::new(3, 2),
                Vector::new(4, 0),
                Vector::new(4, 1),
                Vector::new(4, 2),
                Vector::new(4, 3),
                Vector::new(4, 4),
            ]
        )
    }

    #[test]
    fn test_clear_set() {
        let mut tree = TreePoly::new(5, 5);

        tree.set_leaf(2, 2).unwrap();
        tree.set_leaf(2, 3).unwrap();
        tree.set_leaf(2, 1).unwrap();

        assert_eq!(
            tree.leaves_slow().iter().copied().collect::<Vec<_>>(),
            [Vector::new(2, 1), Vector::new(2, 3)]
        );

        tree.clear_leaf(2, 3).unwrap();

        assert_eq!(
            tree.leaves_slow().iter().copied().collect::<Vec<_>>(),
            [Vector::new(2, 1), Vector::new(2, 2)]
        );

        tree.clear_leaf(2, 1).unwrap();

        assert_eq!(
            tree.leaves_slow().iter().copied().collect::<Vec<_>>(),
            [Vector::new(2, 2)]
        );

        tree.clear_leaf(2, 2).unwrap();
        tree.set_leaf(2, 2).unwrap();
    }

    #[test]
    fn test_substitutable_for() {
        for test_case in [
            r#"
**
*o
            "#,
            r#"
*##
*o.
            "#,
            r#"
####
.*o.
            "#,
            r#"
##.#
.#*#
.#o#
            "#,
            r#"
.#.
###
#o#
            "#,
            r#"
.#..
.###
##o.
            "#,
            r#"
..##
#o.#
####
            "#,
            r#"
#*#
*.o
#*#
            "#,
            r#"
#*#
o.*
#*#
            "#,
            r#"
.#....
##o*#.
*...##
#***#.
            "#,
            r#"
...
.o.
...
            "#,
            r#"
...
.o#
...
            "#,
            r#"
#*#...
*.#o#.
*.#.##
*...*.
*.#.*.
###*#.
.#..#.
            "#,
            r#"
..#..#
#*#o##
*...*.
*.####
*..#.*
*.##.*
*....*
#****#
            "#,
            r#"
#**o***#..
*......*..
*.########
*..#.#.*..
*.##.#.*..
*......*..
#******#..
            "#,
        ] {
            let map: Vec<_> = test_case.split_ascii_whitespace().collect();
            let tree = TreePoly::from_str(&format!(
                "{} {}\n{}",
                map.len(),
                map[0].len(),
                test_case.replace('o', ".").replace('*', "#")
            ))
            .unwrap();

            let v = map
                .iter()
                .enumerate()
                .find_map(|(x, s)| {
                    s.chars()
                        .position(|c| c == 'o')
                        .map(|y| Vector::new(x as i32, y as i32))
                })
                .unwrap();

            let want = map.iter().enumerate().fold(vec![], |mut acc, (x, s)| {
                acc.extend(
                    s.chars()
                        .enumerate()
                        .filter(|(_, c)| *c == '*')
                        .map(|(y, _)| Vector::new(x as i32, y as i32)),
                );
                acc
            });

            let mut got = tree.substitutables_for(v);
            got.sort();

            assert_eq!(got, want, "{:?} {:?} {v:?}", test_case, tree);
        }
    }

    #[test]
    fn test_substitutable_for_alive() {
        for test_case in [
            r#"
*#
#o
            "#,
            r#"
*##
#o.
            "#,
            r#"
##o#
.#*#
.#o#
            "#,
            r#"
.#.
##*
#.#
            "#,
            r#"
.#..
.*##
##..
            "#,
            r#"
..##
#..#
*###
            "#,
            r#"
####o#
#..o.#
##o###
.#o#..
##o##.
#...#.
##*##.
            "#,
            r#"
###
o*o
###
            "#,
            r#"
.#....
##*##.
#...##
##o##.
            "#,
            r#"
...
.*.
...
            "#,
            r#"
...
.*#
...
            "#,
            r#"
###...
#.#o#.
#.#o##
#...#.
#.#o#.
###*#.
.#..#.
            "#,
            r#"
..#..#
###o##
#.o.#.
#o####
#..#.#
#o##.#
#....#
##*###
            "#,
            r#"
###o####..
#.o....#..
#o########
#..#.#.#.#
#o##.#.*.#
#.oo.o.#o#
########o#
            "#,
        ] {
            let map: Vec<_> = test_case.split_ascii_whitespace().collect();
            let tree = TreePoly::from_str(&format!(
                "{} {}\n{}",
                map.len(),
                map[0].len(),
                test_case.replace('o', ".").replace('*', "#")
            ))
            .unwrap();

            let v = map
                .iter()
                .enumerate()
                .find_map(|(x, s)| {
                    s.chars()
                        .position(|c| c == '*')
                        .map(|y| Vector::new(x as i32, y as i32))
                })
                .unwrap();

            let want = map.iter().enumerate().fold(vec![], |mut acc, (x, s)| {
                acc.extend(
                    s.chars()
                        .enumerate()
                        .filter(|(_, c)| *c == 'o')
                        .map(|(y, _)| Vector::new(x as i32, y as i32)),
                );
                acc
            });

            let mut got = tree.substitutables_for(v);
            got.sort();

            assert_eq!(got, want, "{:?} {:?} {v:?}", test_case, tree);
        }
    }

    #[test]
    fn test_try_swap() {
        for test_case in [
            r#"
*##
#o.
            "#,
            r#"
###
#.*
#o#
            "#,
            r#"
###...
#.#o#.
#.#.##
#...#.
#.#.*.
#####.
.#..#.
            "#,
            r#"
#*o#
#..#
####
            "#,
            r#"
#*#o#
#...#
#####
            "#,
            r#"
##*##o##
#......#
########
            "#,
        ] {
            let map: Vec<_> = test_case.split_ascii_whitespace().collect();
            let mut tree = TreePoly::from_str(&format!(
                "{} {}\n{}",
                map.len(),
                map[0].len(),
                test_case.replace('o', ".").replace('*', "#")
            ))
            .unwrap();
            let orig_tree = tree.clone();

            let find = |ch: char| {
                map.iter()
                    .enumerate()
                    .find_map(|(x, s)| {
                        s.chars()
                            .position(|c| c == ch)
                            .map(|y| Vector::new(x as i32, y as i32))
                    })
                    .unwrap()
            };

            let to_set = find('o');
            let to_clear = find('*');

            tree.try_swap(to_set, to_clear).unwrap();

            let want = TreePoly::from_str(&format!(
                "{} {}\n{}",
                map.len(),
                map[0].len(),
                test_case.replace('o', "#").replace('*', ".")
            ))
            .unwrap();

            assert_eq!(tree, want);

            tree.try_swap(to_set, to_clear).unwrap();

            assert_eq!(tree, orig_tree);
        }
    }
}

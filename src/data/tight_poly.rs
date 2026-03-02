use std::{collections::VecDeque, fmt::Display, str::FromStr};

use anyhow::{bail, ensure, Context};

use super::{d4::D4, outline::Outline, rect::Rect, u256::U512, vector::Vector, DX, DY};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TightPoly {
    width: usize,
    rows: Vec<U512>,
}

impl TightPoly {
    pub fn new(rows: Vec<U512>) -> anyhow::Result<Self> {
        ensure!(rows.iter().all(|&row| !row.is_zero()));
        let mask = rows.iter().fold(U512::zero(), |acc, row| acc | *row);
        ensure!(rows.is_empty() || mask.get(0));
        let width = (U512::BITS - mask.leading_zeros()) as usize;

        Ok(Self { width, rows })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.rows.len()
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        x < self.height() && self.rows[x].get(y)
    }

    pub fn unique_rot_revs(&self) -> Vec<Self> {
        let mut res = vec![];

        let mut cur = self.clone();
        for _ in 0..4 {
            cur = cur.roted90();
            res.push(cur.clone());
            res.push(cur.flipped());
        }

        res.sort();
        res.dedup();

        res
    }

    pub fn roted90(&self) -> Self {
        let mut rows = vec![U512::zero(); self.width() as usize];

        for i in 0..self.height() {
            for j in 0..self.width() {
                if self.get(i, j) {
                    rows[(self.width() - 1 - j) as usize].set(i)
                }
            }
        }

        Self::new(rows).unwrap()
    }

    pub fn flipped(&self) -> Self {
        let mut rows = self.rows.clone();
        rows.reverse();
        Self::new(rows).unwrap()
    }

    // Returns whether self intersects other when other is placed at (dx, dy).
    pub fn intersects(&self, other: &TightPoly, dx: i32, dy: i32) -> bool {
        let start_x = dx.max(0);
        let end_x = (dx + other.height() as i32).min(self.height() as i32);

        for x in start_x..end_x {
            let other_x = x - dx;

            let mut mask = self.rows[x as usize];
            let mut other_mask = other.rows[other_x as usize];

            if dy < 0 {
                mask <<= (-dy) as usize;
            } else {
                other_mask <<= dy as usize;
            }

            if !(mask & other_mask).is_zero() {
                return true;
            }
        }

        false
    }

    pub fn cells(&self) -> Vec<Vector> {
        let mut res = vec![];
        for x in 0..self.height() {
            let mut mask = self.rows[x as usize];
            while !mask.is_zero() {
                let y = mask.trailing_zeros() as i32;
                res.push(Vector::new(x as i32, y));
                mask.clear(y as usize);
            }
        }
        res
    }

    pub fn has_hole(&self) -> bool {
        if self.height() <= 2 || self.width() <= 2 {
            return false;
        }

        let cell_count = self.rows.iter().map(|&row| row.count_ones()).sum::<u32>() as usize;

        let mut outer_count = 0usize;

        let mut outer = vec![vec![false; self.width() as usize]; self.height() as usize];
        let mut q = VecDeque::new();

        for x in 0..self.height() {
            for y in [0, self.width() - 1] {
                if !self.get(x, y) {
                    outer[x as usize][y as usize] = true;
                    q.push_back((x, y));
                }
            }
        }
        for y in 1..self.width() - 1 {
            for x in [0, self.height() - 1] {
                if !self.get(x, y) {
                    outer[x as usize][y as usize] = true;
                    q.push_back((x, y));
                }
            }
        }

        let dx = vec![0, 1, 0, -1];
        let dy = vec![1, 0, -1, 0];

        while let Some((x, y)) = q.pop_front() {
            outer_count += 1;

            for d in 0..4 {
                let nx = x as i32 + dx[d];
                let ny = y as i32 + dy[d];

                if nx < 0 || nx >= self.height() as i32 || ny < 0 || ny >= self.width() as i32 {
                    continue;
                }

                if self.get(nx as usize, ny as usize) {
                    continue;
                }

                if outer[nx as usize][ny as usize] {
                    continue;
                }

                outer[nx as usize][ny as usize] = true;
                q.push_back((nx as usize, ny as usize));
            }
        }

        outer_count + cell_count != self.height() * self.width()
    }

    pub fn is_connected(&self) -> bool {
        let cell_count: u32 = self.rows.iter().map(|&row| row.count_ones()).sum();

        let mut comp_count = 0;

        let mut comp = vec![vec![false; self.width() as usize]; self.height() as usize];
        let mut q = VecDeque::new();

        'outer: for x in 0..self.height() {
            for y in 0..self.width() {
                if self.get(x, y) {
                    comp[x as usize][y as usize] = true;
                    q.push_back((x, y));
                    break 'outer;
                }
            }
        }

        while let Some((x, y)) = q.pop_front() {
            comp_count += 1;

            for d in 0..4 {
                let nx = x as i32 + DX[d];
                let ny = y as i32 + DY[d];

                if nx < 0 || nx >= self.height() as i32 || ny < 0 || ny >= self.width() as i32 {
                    continue;
                }

                if !self.get(nx as usize, ny as usize) {
                    continue;
                }

                if comp[nx as usize][ny as usize] {
                    continue;
                }

                comp[nx as usize][ny as usize] = true;
                q.push_back((nx as usize, ny as usize));
            }
        }

        comp_count == cell_count
    }

    pub(crate) fn get_v(&self, v: Vector) -> bool {
        v.x >= 0 && v.y >= 0 && self.get(v.x as usize, v.y as usize)
    }

    // Returns a value between 0 and 1, where 0 means poly is completely symmetric.
    pub fn asymmetricity(&self) -> f64 {
        let mut res = 0.;

        let side = self.height().max(self.width());
        let outline = Outline::new(side, side);

        let offset = Vector::new((side - self.height()) / 2, (side - self.width()) / 2);
        let x_range = offset.x..(side - offset.x);
        let y_range = offset.y..(side - offset.y);

        let trans = |mut x, mut y, d| {
            x += offset.x;
            y += offset.y;
            let (x2, y2) = outline.cell(x, y).transform(d);
            if !x_range.contains(&x2) || !y_range.contains(&y2) {
                return None;
            }
            (x2 - offset.x, y2 - offset.y).into()
        };

        for x in 0..self.height() {
            for y in 0..self.width() {
                let mut diff = 0i32;
                for d in D4::all() {
                    if let Some((x2, y2)) = trans(x, y, d) {
                        diff += if self.get(x2, y2) { 1 } else { -1 };
                    }
                }
                let asymm = match diff.abs() {
                    8 => 0,
                    0 => 1,
                    2 | 6 => 4,
                    4 => 2,
                    _ => unreachable!(),
                };
                res += asymm as f64 / 8.0;

                let mut diff = 0i32;
                for d in [D4::I, D4::R1, D4::R2, D4::R3] {
                    if let Some((x2, y2)) = trans(x, y, d) {
                        diff += if self.get(x2, y2) { 1 } else { -1 };
                    }
                }
                let asymm = match diff.abs() {
                    4 => 0,
                    2 => 4,
                    0 => 2,
                    _ => unreachable!("{x}, {y} {diff} {} {}", self.height(), self.width()),
                };
                res += asymm as f64 / 8.0;
            }
        }
        let res = res / (self.height() * self.width()) as f64;
        assert!(0. <= res && res < 1.);
        res
    }

    pub fn apply(&self, d: D4) -> TightPoly {
        let mut res = if d.get_flip() {
            self.flipped()
        } else {
            self.clone()
        };
        for _ in 0..d.get_rot() {
            res = res.roted90();
        }
        res
    }

    pub(crate) fn bounding_rect(&self) -> Rect {
        let max = Vector::new(self.height() as i32 - 1, self.width() as i32 - 1);
        let min = Vector::new(0, 0);
        Rect::from_min_max(min, max)
    }
}

impl FromStr for TightPoly {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut ss = s.split_ascii_whitespace();
        let height: u32 = ss.next().context("height")?.parse()?;
        let width: usize = ss.next().context("width")?.parse()?;

        let mut rows = vec![];

        for line in ss {
            let mut row = U512::zero();
            for (j, c) in line.chars().enumerate() {
                match c {
                    '.' => {}
                    '#' => row.set(j),
                    _ => bail!("invalid character"),
                }
            }
            rows.push(row);
        }
        ensure!(rows.len() == height as usize);

        let res = Self::new(rows)?;

        ensure!(res.width() == width);

        Ok(res)
    }
}

impl Display for TightPoly {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{} {}", self.height(), self.width())?;
        for row in &self.rows {
            for i in 0..self.width() {
                if row.get(i) {
                    write!(f, "#")?;
                } else {
                    write!(f, ".")?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let empty = TightPoly::new(vec![]).unwrap();
        assert_eq!(empty.width, 0);
    }

    #[test]
    fn test_from_str() {
        let poly = TightPoly::from_str(
            r#"3 4
####
#...
##..
"#,
        )
        .unwrap();
        assert_eq!(
            poly,
            TightPoly::new(vec![0b1111.into(), 0b0001.into(), 0b0011.into()]).unwrap()
        );
    }

    #[test]
    fn test_display() {
        let poly = TightPoly::from_str(
            r#"3 4
####
#...
##..
"#,
        )
        .unwrap();

        assert_eq!(
            poly.to_string(),
            r#"3 4
####
#...
##..
"#,
        );
    }

    #[test]
    fn test_rot90() {
        let poly = TightPoly::from_str(
            r#"2 3
###
..#
"#,
        )
        .unwrap();
        assert_eq!(
            poly.roted90(),
            TightPoly::from_str(
                r#"3 2
##
#.
#.
"#
            )
            .unwrap()
        );
    }

    #[test]
    fn test_unique_rot_revs() {
        assert_eq!(
            TightPoly::from_str(
                r#"2 3
###
..#
"#,
            )
            .unwrap()
            .unique_rot_revs()
            .len(),
            8
        );

        assert_eq!(
            TightPoly::from_str(
                r#"2 3
##.
.##
"#,
            )
            .unwrap()
            .unique_rot_revs()
            .len(),
            4
        );

        assert_eq!(
            TightPoly::from_str(
                r#"1 2
##
"#,
            )
            .unwrap()
            .unique_rot_revs()
            .len(),
            2
        );

        assert_eq!(
            TightPoly::from_str(
                r#"2 2
##
##
"#,
            )
            .unwrap()
            .unique_rot_revs()
            .len(),
            1
        );
    }

    #[test]
    fn test_intersects() {
        let p1 = TightPoly::from_str(
            r#"3 4
####
#..#
####
"#,
        )
        .unwrap();

        let p2 = TightPoly::from_str(
            r#"1 2
##
"#,
        )
        .unwrap();

        assert!(p1.intersects(&p2, 0, 0));
        assert!(!p1.intersects(&p2, 1, 1));
        assert!(!p1.intersects(&p2, -1, -1));
        assert!(!p1.intersects(&p2, -1, 0));
        assert!(p1.intersects(&p2, 0, -1));
        assert!(p1.intersects(&p2, 0, 3));
        assert!(!p1.intersects(&p2, 0, 4));
        assert!(p1.intersects(&p2, 2, 3));
        assert!(!p1.intersects(&p2, 3, 3));
    }

    #[test]
    fn test_cells() {
        let p = TightPoly::from_str(
            r#"3 4
####
##.#
#..#
"#,
        )
        .unwrap();

        assert_eq!(
            p.cells(),
            vec![
                Vector::new(0, 0),
                Vector::new(0, 1),
                Vector::new(0, 2),
                Vector::new(0, 3),
                Vector::new(1, 0),
                Vector::new(1, 1),
                Vector::new(1, 3),
                Vector::new(2, 0),
                Vector::new(2, 3)
            ]
        );
    }

    #[test]
    fn test_has_hole() {
        for (s, want) in [
            (
                r#"3 3
##.
#..
###
"#,
                false,
            ),
            (
                r#"3 3
##.
#.#
###
"#,
                true,
            ),
            (
                r#"3 4
####
#..#
.###
"#,
                true,
            ),
            (
                r#"3 3
###
###
###
"#,
                false,
            ),
            (
                r#"4 5
.#.#.
##.##
#...#
#####
"#,
                false,
            ),
        ] {
            let p = TightPoly::from_str(s).unwrap();
            assert_eq!(p.has_hole(), want);
        }
    }

    #[test]
    fn test_is_connected() {
        for (s, want) in [
            (
                r#"3 3
##.
#.#
###
"#,
                true,
            ),
            (
                r#"3 3
##.
#.#
.##
"#,
                false,
            ),
            (
                r#"3 4
####
#...
.###
"#,
                false,
            ),
            (
                r#"3 3
###
###
###
"#,
                true,
            ),
            (
                r#"4 5
.#.#.
##.##
#...#
#####
"#,
                true,
            ),
        ] {
            let p = TightPoly::from_str(s).unwrap();
            assert_eq!(p.is_connected(), want, "{s}");
        }
    }
}

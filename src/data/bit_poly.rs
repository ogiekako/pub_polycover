use std::fmt::Display;

use crate::data::u256::U512;

use super::{rect::Rect, vector::Vector};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BitPoly {
    pub rows: Vec<U512>,
    pub width: usize,

    pub x_nonzero: U512,
    pub y_count: Vec<usize>,
    pub y_nonzero: U512,

    cell_count: usize,
}

impl BitPoly {
    pub fn new(height: usize, width: usize) -> Self {
        assert!(width < U512::BITS as usize);
        Self {
            rows: vec![0.into(); height as usize],

            x_nonzero: 0.into(),
            y_count: vec![0; width],
            y_nonzero: 0.into(),

            width,
            cell_count: 0,
        }
    }

    pub fn height(&self) -> usize {
        self.rows.len()
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn cell_count(&self) -> usize {
        self.cell_count
    }

    pub fn is_empty(&self) -> bool {
        self.cell_count == 0
    }

    // Returns the value after flipping.
    pub fn flip(&mut self, v: &Vector) -> bool {
        let (x, y) = (v.x as usize, v.y as usize);

        debug_assert!(x < self.height());
        debug_assert!(y < self.width());

        let row = &mut self.rows[x];

        if row.is_zero() {
            self.x_nonzero.set(x);
        }

        row.flip(y);
        let set = row.get(y);
        if set {
            if self.y_count[y] == 0 {
                self.y_nonzero.set(y);
            }
            self.cell_count += 1;
            self.y_count[y] += 1;
        } else {
            self.cell_count -= 1;
            self.y_count[y] -= 1;

            if row.is_zero() {
                self.x_nonzero.clear(x);
            }
            if self.y_count[y] == 0 {
                self.y_nonzero.clear(y);
            }
        }

        set
    }

    #[inline(always)]
    pub(crate) fn get(&self, x: usize, y: usize) -> bool {
        x < self.height() && self.rows[x].get(y)
    }

    #[inline(always)]
    pub(crate) fn get_v(&self, v: Vector) -> bool {
        v.x >= 0 && v.y >= 0 && self.get(v.x as usize, v.y as usize)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = Vector> + '_ {
        Iter::new(self)
    }

    pub fn bounding_box(&self) -> Rect {
        if self.is_empty() {
            return Rect::new(Vector::new(0, 0), Vector::new(0, 0));
        }
        let min_x = self.x_nonzero.trailing_zeros() as i32;
        let min_y = self.y_nonzero.trailing_zeros() as i32;
        let max_x = (U512::BITS - 1 - self.x_nonzero.leading_zeros()) as i32;
        let max_y = (U512::BITS - 1 - self.y_nonzero.leading_zeros()) as i32;

        Rect::from_min_max(Vector::new(min_x, min_y), Vector::new(max_x, max_y))
    }
}

struct Iter<'a> {
    m: u128,
    x: usize,
    y: usize,

    rows: &'a [U512],
}

impl<'a> Iter<'a> {
    pub fn new(v: &'a BitPoly) -> Self {
        Self {
            m: v.rows[v.rows.len() - 1].data()[0],
            x: v.rows.len() - 1,
            y: 0,
            rows: &v.rows,
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Vector;

    fn next(&mut self) -> Option<Self::Item> {
        while self.m == 0 {
            self.m = if self.y == U512::BITS as usize / 128 - 1 {
                if self.x == 0 {
                    return None;
                }
                self.x -= 1;
                self.y = 0;
                unsafe { self.rows.get_unchecked(self.x) }.data()[0]
            } else {
                self.y += 1;
                unsafe { self.rows.get_unchecked(self.x) }.data()[self.y]
            }
        }
        let y = self.m.trailing_zeros() as i32;
        self.m &= !(1 << y);

        Some(Vector::new(self.x as i32, (self.y as i32) << 7 | y))
    }
}

impl Display for BitPoly {
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

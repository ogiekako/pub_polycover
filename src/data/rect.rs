use std::{fmt::Debug, ops::Range};

use super::{d4::D4, e2::E2, vector::Vector};

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rect {
    origin: Vector,
    diag: Vector,
}

impl Debug for Rect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let xs = self.x_range();
        let ys = self.y_range();
        write!(f, "{:?} x {:?}", xs, ys)
    }
}

impl Rect {
    pub fn new(origin: Vector, diag: Vector) -> Self {
        Self { origin, diag }
    }

    pub fn from_min_max(min: Vector, max: Vector) -> Self {
        Self::new(min, max - &min + &Vector::new(1, 1))
    }

    pub fn roted90(&self, count: usize) -> Self {
        Self::new(self.origin.roted90(count), self.diag.roted90(count))
    }

    pub fn rot90(&mut self, count: usize) {
        self.origin.rot90(count);
        self.diag.rot90(count);
    }

    pub fn flipped(&self) -> Self {
        Self::new(self.origin.flipped(), self.diag.flipped())
    }

    pub fn moved(&self, delta: &Vector) -> Self {
        Self::new(&self.origin + delta, self.diag)
    }

    pub fn x_range(&self) -> Range<i32> {
        if self.diag.x > 0 {
            self.origin.x..self.origin.x + self.diag.x
        } else {
            self.origin.x + self.diag.x + 1..self.origin.x + 1
        }
    }

    pub fn min_corner(&self) -> Vector {
        Vector::new(self.x_range().start, self.y_range().start)
    }

    pub fn max_corner(&self) -> Vector {
        Vector::new(self.x_range().end - 1, self.y_range().end - 1)
    }

    pub fn y_range(&self) -> Range<i32> {
        if self.diag.y > 0 {
            self.origin.y..self.origin.y + self.diag.y
        } else {
            self.origin.y + self.diag.y + 1..self.origin.y + 1
        }
    }

    pub fn intersection(&self, other: &Rect) -> Rect {
        let x_range = intersection(self.x_range(), other.x_range());
        let y_range = intersection(self.y_range(), other.y_range());
        Rect::new(
            Vector::new(x_range.start, y_range.start),
            Vector::new(x_range.end - x_range.start, y_range.end - y_range.start),
        )
    }

    pub fn contains(&self, point: &Vector) -> bool {
        self.x_range().contains(&point.x) && self.y_range().contains(&point.y)
    }

    pub fn area(&self) -> u32 {
        self.diag.x.unsigned_abs() * self.diag.y.unsigned_abs()
    }

    pub fn is_empty(&self) -> bool {
        self.diag.x == 0 || self.diag.y == 0
    }

    pub fn height(&self) -> usize {
        self.diag.x.unsigned_abs() as usize
    }

    pub fn width(&self) -> usize {
        self.diag.y.unsigned_abs() as usize
    }
}

fn intersection(a: Range<i32>, b: Range<i32>) -> Range<i32> {
    let start = a.start.max(b.start);
    let end = a.end.min(b.end);
    if start < end {
        start..end
    } else {
        start..start
    }
}

impl std::ops::Mul<&Rect> for D4 {
    type Output = Rect;

    fn mul(self, rhs: &Rect) -> Self::Output {
        (if self.get_flip() { rhs.flipped() } else { *rhs }).roted90(self.get_rot())
    }
}

impl std::ops::Mul<&Rect> for &E2 {
    type Output = Rect;

    fn mul(self, rhs: &Rect) -> Self::Output {
        &self.d + &(self.r * rhs)
    }
}

impl std::ops::Mul<Rect> for E2 {
    type Output = Rect;

    fn mul(self, rhs: Rect) -> Self::Output {
        &self.d + &(self.r * &rhs)
    }
}

impl std::ops::Add<&Rect> for &Vector {
    type Output = Rect;

    fn add(self, rhs: &Rect) -> Self::Output {
        rhs.moved(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect() {
        let rect = Rect::new(Vector::new(0, 0), Vector::new(3, 4));
        assert_eq!(rect.x_range(), 0..3);
        assert_eq!(rect.y_range(), 0..4);

        let rect = Rect::new(Vector::new(0, 0), Vector::new(-3, -4));
        assert_eq!(rect.x_range(), -2..1);
        assert_eq!(rect.y_range(), -3..1);
    }

    #[test]
    fn test_intersection() {
        let rect1 = Rect::new(Vector::new(1, 1), Vector::new(2, 3));

        let mut rect2 = rect1.roted90(1);
        assert_eq!(rect2.origin, (-1, 1).into());

        rect2 = rect2.moved(&Vector::new(2, 0));

        let rect3 = rect1.intersection(&rect2);

        assert_eq!(rect3.x_range(), 1..2);
        assert_eq!(rect3.y_range(), 1..3);
    }
}

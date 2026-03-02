use std::fmt::Debug;

use super::{DX, DY};

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Vector<T = i32> {
    pub x: T,
    pub y: T,
}

impl<T: Debug> Debug for Vector<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:?},{:?})", self.x, self.y)
    }
}

impl<T> From<(T, T)> for Vector<T> {
    fn from(val: (T, T)) -> Self {
        Vector::new(val.0, val.1)
    }
}

pub const DIR4: [Vector; 4] = [
    Vector { x: DX[0], y: DY[0] },
    Vector { x: DX[1], y: DY[1] },
    Vector { x: DX[2], y: DY[2] },
    Vector { x: DX[3], y: DY[3] },
];

pub const DIR8: [Vector; 8] = [
    Vector::new(0, 1),
    Vector::new(1, 1),
    Vector::new(1, 0),
    Vector::new(1, -1),
    Vector::new(0, -1),
    Vector::new(-1, -1),
    Vector::new(-1, 0),
    Vector::new(-1, 1),
];

impl<T> Vector<T> {
    pub const fn new(x: T, y: T) -> Self {
        Self { x, y }
    }
}

impl Vector {
    pub(crate) fn neighbors4(&self) -> impl Iterator<Item = Self> + '_ {
        [
            Vector::new(self.x, self.y + 1),
            Vector::new(self.x + 1, self.y),
            Vector::new(self.x, self.y - 1),
            Vector::new(self.x - 1, self.y),
        ]
        .into_iter()
    }
    pub(crate) fn neighbors8(&self) -> impl Iterator<Item = Self> + '_ {
        DIR8.iter().map(move |dir| self + dir)
    }

    pub(crate) fn directions8() -> impl Iterator<Item = Self> {
        DIR8.iter().copied()
    }
}

impl<T> Vector<T>
where
    T: std::ops::Add<T, Output = T> + Copy + Ord + std::ops::Neg<Output = T>,
{
    pub(crate) fn flipped(&self) -> Self {
        Self::new(-self.x, self.y)
    }

    pub(crate) fn roted90(&self, count: usize) -> Self {
        match count & 3 {
            0 => *self,
            1 => Self::new(-self.y, self.x),
            2 => Self::new(-self.x, -self.y),
            3 => Self::new(self.y, -self.x),
            _ => unreachable!(),
        }
    }

    pub fn rot90(&mut self, count: usize) {
        match count & 3 {
            0 => (),
            1 => {
                let tmp = self.x;
                self.x = -self.y;
                self.y = tmp;
            }
            2 => {
                self.x = -self.x;
                self.y = -self.y;
            }
            3 => {
                let tmp = self.x;
                self.x = self.y;
                self.y = -tmp;
            }
            _ => unreachable!(),
        }
    }
}

impl<T> Vector<T>
where
    T: std::ops::Add<T, Output = T> + Copy + Ord,
{
    pub fn pairwise_min(&self, rhs: &Vector<T>) -> Vector<T> {
        Self::new(self.x.min(rhs.x), self.y.min(rhs.y))
    }

    pub fn pairwise_max(&self, rhs: &Vector<T>) -> Vector<T> {
        Self::new(self.x.max(rhs.x), self.y.max(rhs.y))
    }
}

impl<T> std::ops::Sub<Vector<T>> for Vector<T>
where
    T: std::ops::SubAssign<T>,
{
    type Output = Vector<T>;

    fn sub(mut self, rhs: Vector<T>) -> Self::Output {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self
    }
}

impl<T> std::ops::Sub<&Vector<T>> for &Vector<T>
where
    T: std::ops::Sub<T, Output = T> + Copy,
{
    type Output = Vector<T>;

    fn sub(self, rhs: &Vector<T>) -> Self::Output {
        Vector::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl<'a, T> std::ops::Sub<&'a Vector<T>> for Vector<T>
where
    T: std::ops::SubAssign<&'a T>,
{
    type Output = Vector<T>;

    fn sub(mut self, rhs: &'a Vector<T>) -> Self::Output {
        self.x -= &rhs.x;
        self.y -= &rhs.y;
        self
    }
}

impl<T> std::ops::Add<Vector<T>> for Vector<T>
where
    T: std::ops::AddAssign<T>,
{
    type Output = Vector<T>;

    fn add(mut self, rhs: Vector<T>) -> Self::Output {
        self.x += rhs.x;
        self.y += rhs.y;
        self
    }
}

impl<'a, T> std::ops::Add<&'a Vector<T>> for Vector<T>
where
    T: std::ops::AddAssign<&'a T>,
{
    type Output = Vector<T>;

    fn add(mut self, rhs: &'a Vector<T>) -> Self::Output {
        self.x += &(rhs.x);
        self.y += &(rhs.y);
        self
    }
}

impl<T> std::ops::Add for &Vector<T>
where
    T: std::ops::Add<T, Output = T> + Copy,
{
    type Output = Vector<T>;

    fn add(self, rhs: &Vector<T>) -> Self::Output {
        Vector::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl<T> std::ops::SubAssign<Vector<T>> for Vector<T>
where
    T: std::ops::SubAssign<T>,
{
    fn sub_assign(&mut self, rhs: Vector<T>) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl<T> std::ops::AddAssign<Vector<T>> for Vector<T>
where
    T: std::ops::AddAssign<T>,
{
    fn add_assign(&mut self, rhs: Vector<T>) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl<T> std::ops::Neg for Vector<T>
where
    T: std::ops::Neg<Output = T>,
{
    type Output = Vector<T>;

    fn neg(mut self) -> Self::Output {
        self.x = -self.x;
        self.y = -self.y;
        self
    }
}

impl<T> std::ops::Neg for &Vector<T>
where
    T: std::ops::Neg<Output = T> + Copy,
{
    type Output = Vector<T>;

    fn neg(self) -> Self::Output {
        Vector::new(-self.x, -self.y)
    }
}

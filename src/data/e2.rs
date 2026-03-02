use super::{d4::D4, vector::Vector};

// https://en.wikipedia.org/wiki/Euclidean_group
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct E2 {
    pub r: D4,
    pub d: Vector,
}

impl std::fmt::Debug for E2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}{:?}", self.d, self.r)
    }
}

impl E2 {
    pub fn new(r: D4, d: Vector) -> Self {
        Self { r, d }
    }

    pub(crate) fn inverse(&self) -> Self {
        let r = self.r.inverse();
        let d = r * -self.d;
        Self::new(r, d)
    }
}

impl From<(D4, Vector)> for E2 {
    fn from((r, d): (D4, Vector)) -> Self {
        Self::new(r, d)
    }
}

impl std::ops::Mul for &E2 {
    type Output = E2;

    fn mul(self, rhs: Self) -> Self::Output {
        E2::new(self.r * rhs.r, &self.d + &(self.r * rhs.d))
    }
}

impl std::ops::Mul for E2 {
    type Output = E2;

    fn mul(self, rhs: Self) -> Self::Output {
        E2::new(self.r * rhs.r, &self.d + &(self.r * rhs.d))
    }
}

impl std::ops::Mul<&Vector> for &E2 {
    type Output = Vector;

    fn mul(self, rhs: &Vector) -> Self::Output {
        &self.r * rhs + &self.d
    }
}

impl std::ops::Mul<Vector> for E2 {
    type Output = Vector;

    fn mul(self, rhs: Vector) -> Self::Output {
        &self.r * &rhs + &self.d
    }
}

impl std::ops::Add<&E2> for &Vector {
    type Output = E2;

    fn add(self, rhs: &E2) -> Self::Output {
        E2::new(rhs.r, self + &rhs.d)
    }
}

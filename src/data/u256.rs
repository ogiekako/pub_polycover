use std::ops::{BitAnd, BitOr, Shl, ShlAssign, Shr, ShrAssign};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord)]
pub struct Us<const N: usize> {
    data: [u128; N],
}

pub type U128 = Us<1>;

pub type U256 = Us<2>;

pub type U512 = Us<4>;

impl<const N: usize> Us<N> {
    pub const BITS: u32 = N as u32 * 128;

    pub fn data(&self) -> &[u128; N] {
        &self.data
    }

    pub fn flip(&mut self, i: usize) {
        self.data[i / 128] ^= 1 << (i % 128);
    }

    pub fn trailing_zeros(&self) -> u32 {
        let mut offset = 0;
        for d in self.data.iter() {
            if *d != 0 {
                return offset + d.trailing_zeros();
            }
            offset += 128;
        }
        offset
    }

    pub fn leading_zeros(&self) -> u32 {
        let mut offset = 0;
        for d in self.data.iter().rev() {
            if *d != 0 {
                return offset + d.leading_zeros();
            }
            offset += 128;
        }
        offset
    }

    pub fn is_zero(&self) -> bool {
        self.data.iter().all(|x| *x == 0)
    }

    pub fn get(&self, i: usize) -> bool {
        self.data[i >> 7] >> (i & 127) & 1 != 0
    }

    pub(crate) fn zero() -> Self {
        Self { data: [0; N] }
    }

    pub(crate) fn set(&mut self, i: usize) {
        self.data[i >> 7] |= 1 << (i & 127);
    }

    pub(crate) fn clear(&mut self, i: usize) {
        self.data[i >> 7] &= !(1 << (i & 127));
    }

    pub(crate) fn count_ones(&self) -> u32 {
        self.data.iter().map(|x| x.count_ones()).sum()
    }

    pub fn count_ones_lt(&self, i: usize) -> u32 {
        assert!(i < 128 * N);

        let mut res = 0;
        for d in self.data.iter().take(i >> 7) {
            res += d.count_ones();
        }
        res + (self.data[i >> 7] & ((1 << (i & 127)) - 1)).count_ones()
    }

    pub fn count_ones_gt(&self, i: usize) -> u32 {
        assert!(i < 128 * N);

        let mut res = 0;
        for d in self.data.iter().skip((i + 1 >> 7) + 1) {
            res += d.count_ones();
        }
        res + (self.data[i + 1 >> 7] >> (i + 1 & 127)).count_ones()
    }

    fn new(data: [u128; N]) -> Self {
        Self { data }
    }
}

impl BitOr for U512 {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::new([
            self.data[0] | rhs.data[0],
            self.data[1] | rhs.data[1],
            self.data[2] | rhs.data[2],
            self.data[3] | rhs.data[3],
        ])
    }
}

impl BitAnd for U512 {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self::new([
            self.data[0] & rhs.data[0],
            self.data[1] & rhs.data[1],
            self.data[2] & rhs.data[2],
            self.data[3] & rhs.data[3],
        ])
    }
}

impl<const N: usize> From<u128> for Us<N> {
    fn from(v: u128) -> Self {
        let mut res = [0; N];
        res[0] = v;
        Self::new(res)
    }
}

impl<const N: usize> Shl<usize> for &Us<N> {
    type Output = Us<N>;

    fn shl(self, rhs: usize) -> Self::Output {
        let mut res = [0; N];
        for i in 0..N - (rhs >> 7) {
            res[(rhs >> 7) + i] = self.data[i] << (rhs & 127);
            if i > 0 && rhs & 127 != 0 {
                res[(rhs >> 7) + i] |= self.data[i - 1] >> (Self::Output::BITS as usize - rhs & 127)
            };
        }
        Self::Output::new(res)
    }
}

impl<const N: usize> Shr<usize> for &Us<N> {
    type Output = Us<N>;

    fn shr(self, rhs: usize) -> Self::Output {
        let mut res = [0; N];
        for i in 0..N - (rhs >> 7) {
            res[i] = self.data[i + (rhs >> 7)] >> (rhs & 127)
                | if rhs & 127 != 0 && i + 1 + (rhs >> 7) < N {
                    self.data[i + 1 + (rhs >> 7)] << (Self::Output::BITS as usize - rhs & 127)
                } else {
                    0
                };
        }
        Self::Output::new(res)
    }
}

impl ShlAssign<usize> for U512 {
    fn shl_assign(&mut self, rhs: usize) {
        *self = &*self << rhs;
    }
}

impl ShrAssign<usize> for U512 {
    fn shr_assign(&mut self, rhs: usize) {
        *self = &*self >> rhs;
    }
}

impl<const N: usize> PartialOrd for Us<N> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        for i in (1..N).rev() {
            match self.data[i].partial_cmp(&other.data[i]) {
                Some(core::cmp::Ordering::Equal) => {}
                ord => return ord,
            }
        }
        self.data[0].partial_cmp(&other.data[0])
    }
}

#[cfg(test)]
mod tests {
    use crate::data::u256::Us;

    #[test]
    fn test_set_clear() {
        let mut u = super::U512::zero();
        u.set(0);
        assert_eq!(u.data[0], 1);
        u.set(127);
        assert_eq!(u.data[0], 1 | 1 << 127);
        assert_eq!(u.data[1], 0);
        u.set(128);
        assert_eq!(u.data[0], 1 | 1 << 127);
        assert_eq!(u.data[1], 1);

        u.set(255);
        assert_eq!(u.data[0], 1 | 1 << 127);
        assert_eq!(u.data[1], 1 | 1 << 127);

        assert!(u.get(0));
        assert!(u.get(127));
        assert!(u.get(128));
        assert!(u.get(255));
        assert!(!u.get(254));

        u.clear(0);
        assert_eq!(u.data[0], 1 << 127);
        assert_eq!(u.data[1], 1 | 1 << 127);
        u.clear(127);
        assert_eq!(u.data[0], 0);
        assert_eq!(u.data[1], 1 | 1 << 127);
        u.clear(128);
        assert_eq!(u.data[0], 0);
        assert_eq!(u.data[1], 1 << 127);
        u.clear(255);
        assert_eq!(u.data[0], 0);
        assert_eq!(u.data[1], 0);
    }

    #[test]
    fn test_shifts() {
        type U = Us<3>;
        let mut u = U::zero();
        u.set(0);
        assert_eq!(&u << 1, U::new([2, 0, 0]));
        assert_eq!(&u << 127, U::new([1 << 127, 0, 0]));
        assert_eq!(&u << 128, U::new([0, 1, 0]));
        assert_eq!(&u << 129, U::new([0, 2, 0]));
        assert_eq!(&u << 255, U::new([0, 1 << 127, 0]));
        assert_eq!(&u << 360, U::new([0, 0, 1 << 104]));

        u.set(128);
        assert_eq!(&u << 1, U::new([2, 2, 0]));
        assert_eq!(&u << 128, U::new([0, 1, 1]));
        assert_eq!(&u << 255, U::new([0, 1 << 127, 1 << 127]));

        u.set(255);
        assert_eq!(&u >> 1, U::new([1 << 127, 1 << 126, 0]));
        assert_eq!(&u >> 128, U::new([1 | 1 << 127, 0, 0]));
    }
}

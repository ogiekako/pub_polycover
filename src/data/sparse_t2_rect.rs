use super::vector::Vector;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SparseT2Rect {
    height: usize,
    width: usize,
    x_shift: usize,
    components: Vec<u16>,
}

impl SparseT2Rect {
    pub fn new(mut height: usize, mut width: usize) -> Self {
        assert!(height % 4 == 0);
        assert!(width % 4 == 0);
        height /= 4;
        width /= 4;
        let x_shift = width.next_power_of_two().trailing_zeros() as usize;
        Self {
            height,
            width,
            x_shift,
            components: vec![0; height << x_shift],
        }
    }

    pub fn height(&self) -> usize {
        self.height << 2
    }

    pub fn width(&self) -> usize {
        self.width << 2
    }

    pub fn flip(&mut self, v: Vector<usize>) {
        debug_assert!(
            (0..self.height()).contains(&v.x),
            "{} {}",
            self.height(),
            v.x
        );
        debug_assert!((0..self.width()).contains(&v.y), "{} {}", self.width(), v.y);

        self.components[(v.x >> 2) << self.x_shift | (v.y >> 2)] ^= 1 << ((v.x & 3) << 2 | v.y & 3);
    }

    pub(crate) fn clear(&mut self, v: &Vector<usize>) {
        self.components[(v.x >> 2) << self.x_shift | (v.y >> 2)] &=
            !(1 << ((v.x & 3) << 2 | v.y & 3));
    }

    pub(crate) fn set(&mut self, v: &Vector<usize>) {
        self.components[(v.x >> 2) << self.x_shift | (v.y >> 2)] |= 1 << ((v.x & 3) << 2 | v.y & 3);
    }

    pub fn get(&self, v: &Vector) -> bool {
        debug_assert!(v.x < self.height() as i32);
        debug_assert!(v.y < self.width() as i32);

        self.components[((v.x >> 2) << self.x_shift as i32 | (v.y >> 2)) as usize]
            >> ((v.x & 3) << 2 | v.y & 3)
            & 1
            != 0
    }

    pub fn iter(&self) -> impl Iterator<Item = Vector> + '_ {
        Iter::new(self)
    }

    pub fn and_iter<'a>(
        &'a self,
        origin: Vector<usize>,
        rhs: &'a SparseT2Rect,
    ) -> impl Iterator<Item = Vector> + 'a {
        AndIter::new(self, origin, rhs)
    }
}

struct AndIter<'a> {
    i1: usize,
    i2: usize,

    x_shift1: usize,

    m: u16,
    y2: u16,
    w2m1: u16,

    wrap1: usize,
    wrap2: usize,

    c: &'a [u16],
    c2: &'a [u16],
}

impl<'a> AndIter<'a> {
    fn new(large: &'a SparseT2Rect, origin: Vector<usize>, small: &'a SparseT2Rect) -> Self {
        debug_assert_eq!(origin.x & 3, 0);
        debug_assert_eq!(origin.y & 3, 0);

        let i = (origin.x >> 2) << large.x_shift as usize | (origin.y >> 2);
        let w2m1 = small.width as u16 - 1;

        Self {
            i1: i,
            i2: 0,
            x_shift1: large.x_shift,
            m: large.components[i] & small.components[0],
            y2: w2m1 as u16,
            wrap1: ((1 << large.x_shift) - w2m1) as usize,
            wrap2: ((1 << small.x_shift) - w2m1) as usize,
            w2m1,
            c: &large.components,
            c2: &small.components,
        }
    }
}

impl<'a> Iterator for AndIter<'a> {
    type Item = Vector;

    fn next(&mut self) -> Option<Self::Item> {
        while self.m == 0 {
            if self.y2 == 0 {
                self.i2 += self.wrap2;
                if self.i2 >= self.c2.len() {
                    return None;
                }
                self.i1 += self.wrap1;
                self.y2 = self.w2m1;
            } else {
                self.i1 += 1;
                self.i2 += 1;
                self.y2 -= 1;
            }

            self.m = unsafe { self.c.get_unchecked(self.i1) & self.c2.get_unchecked(self.i2) };
        }
        let k = self.m.trailing_zeros() as usize;
        self.m &= !(1 << k);

        let (x, y) = (
            self.i1 >> self.x_shift1,
            self.i1 & !(usize::MAX << self.x_shift1),
        );

        Some(Vector::new(
            (x << 2 | (k >> 2)) as i32,
            (y << 2 | (k & 3)) as i32,
        ))
    }
}

struct Iter<'a> {
    i: usize,
    m: u16,

    sub: usize,
    x_shift: u32,
    c: &'a [u16],
}

impl<'a> Iter<'a> {
    pub fn new(v: &'a SparseT2Rect) -> Self {
        let i = v.components.len() - 1;
        Self {
            i,
            m: v.components[i],
            sub: (1 << v.x_shift) - v.width,
            x_shift: v.x_shift as u32,
            c: &v.components,
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Vector;

    fn next(&mut self) -> Option<Self::Item> {
        while self.m == 0 {
            if self.i == 0 {
                return None;
            }
            self.i -= 1;
            if (usize::MAX << self.x_shift) | self.i == usize::MAX {
                self.i -= self.sub;
            }

            self.m = *unsafe { self.c.get_unchecked(self.i) };
        }
        let k = self.m.trailing_zeros() as i32;
        self.m &= !(1 << k);

        let (x, y) = (
            self.i >> self.x_shift,
            self.i & !(usize::MAX << self.x_shift),
        );

        Some(Vector::new(
            (x << 2) as i32 | k >> 2,
            (y << 2) as i32 | k & 3,
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::data::vector::Vector;

    #[test]
    fn test_and_iter() {
        use super::SparseT2Rect;

        let mut a = SparseT2Rect::new(16, 16);
        let mut b = SparseT2Rect::new(16, 16);

        a.flip(Vector::new(0, 0));
        a.flip(Vector::new(0, 1));
        a.flip(Vector::new(1, 0));
        a.flip(Vector::new(1, 1));
        a.flip(Vector::new(8, 9));
        a.flip(Vector::new(9, 9));
        a.flip(Vector::new(1, 15));

        b.flip(Vector::new(0, 0));
        b.flip(Vector::new(0, 1));
        b.flip(Vector::new(1, 1));
        b.flip(Vector::new(2, 0));
        b.flip(Vector::new(8, 9));
        b.flip(Vector::new(9, 10));
        b.flip(Vector::new(1, 15));

        let mut iter = a.and_iter((0, 0).into(), &b);
        assert_eq!(iter.next(), Some((0, 0).into()));
        assert_eq!(iter.next(), Some((0, 1).into()));
        assert_eq!(iter.next(), Some((1, 1).into()));
        assert_eq!(iter.next(), Some((1, 15).into()));
        assert_eq!(iter.next(), Some((8, 9).into()));
        assert_eq!(iter.next(), None);
    }
}

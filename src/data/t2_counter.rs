use super::vector::Vector;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct T2Counter {
    min: Vector<isize>,
    max: Vector,

    height: usize,
    x_shift: usize,

    counter: Vec<usize>,
}

impl T2Counter {
    pub fn new(min: Vector, max: Vector) -> Self {
        let height = (max.x - min.x + 1) as usize;
        let x_shift = ((max.y - min.y + 1) as usize)
            .next_power_of_two()
            .trailing_zeros() as usize;

        Self {
            min: Vector::new(min.x as isize, min.y as isize),
            max,
            height,
            x_shift,
            counter: vec![0; height << x_shift],
        }
    }

    pub fn min(&self) -> &Vector<isize> {
        &self.min
    }

    pub fn get(&self, v: &Vector<isize>) -> Option<usize> {
        if v.x < self.min.x
            || v.x > self.max.x as isize
            || v.y < self.min.y
            || v.y > self.max.y as isize
        {
            None
        } else {
            Some(self.get_unchecked(v))
        }
    }

    pub fn get_unchecked(&self, v: &Vector<isize>) -> usize {
        *unsafe { self.counter.get_unchecked(self.index(v)) }
    }

    pub fn get_mut(&mut self, v: &Vector<isize>) -> &mut usize {
        let i = self.index(v);
        unsafe { self.counter.get_unchecked_mut(i) }
    }

    pub fn get_raw_mut(&mut self, v: &Vector<usize>) -> &mut usize {
        let i = v.x << self.x_shift | v.y;
        debug_assert!(i < self.counter.len());
        unsafe { self.counter.get_unchecked_mut(i) }
    }

    fn index(&self, v: &Vector<isize>) -> usize {
        debug_assert!((self.min.x as i32..=self.max.x).contains(&(v.x as i32)));
        debug_assert!((self.min.y as i32..=self.max.y).contains(&(v.y as i32)));

        ((v.x - self.min.x) << self.x_shift | (v.y - self.min.y)) as usize
    }
}

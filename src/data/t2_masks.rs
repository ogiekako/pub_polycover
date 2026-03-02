use super::{sparse_t2_rect::SparseT2Rect, vector::Vector};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct T2Masks {
    min: Vector,
    max: Vector,

    height: usize,
    width: usize,

    x_shift: usize,

    masks: Vec<usize>,

    rects: Vec<SparseT2Rect>,
}

impl T2Masks {
    pub fn new(min: Vector, max: Vector, full_mask: usize) -> Self {
        let height = (max.x - min.x + 1) as usize;
        let width = (max.y - min.y + 1) as usize;
        let x_shift = width.next_power_of_two().trailing_zeros() as usize;

        let masks = vec![0; height << x_shift];
        let rects = vec![SparseT2Rect::new(height, width); full_mask + 1];
        Self {
            min: Vector::new(min.x, min.y),
            max: Vector::new(max.x, max.y),
            height,
            width,
            x_shift,
            masks,
            rects,
        }
    }

    pub fn min(&self) -> &Vector {
        &self.min
    }

    pub fn max(&self) -> &Vector {
        &self.max
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn width(&self) -> usize {
        self.width
    }

    // Returns flipped mask
    pub fn flip(&mut self, v: &Vector, i: usize) -> usize {
        let p = Vector::new(
            (v.x as isize - self.min.x as isize) as usize,
            (v.y as isize - self.min.y as isize) as usize,
        );

        let m = &mut self.masks[p.x << self.x_shift | p.y];

        self.rects[*m].clear(&p);

        *m ^= i;

        self.rects[*m].set(&p);

        *m
    }

    pub fn get_raw(&self, x: usize, y: usize) -> usize {
        debug_assert!((x << self.x_shift | y) < self.masks.len());
        *unsafe { self.masks.get_unchecked(x << self.x_shift | y) }
    }

    pub(crate) fn rect(&self, mask: usize) -> &SparseT2Rect {
        &self.rects[mask]
    }
}

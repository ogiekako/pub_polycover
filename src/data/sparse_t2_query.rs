use std::ops::Range;

use super::{sparse_t2_rect::SparseT2Rect, vector::Vector};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseT2Query {
    min: Vector,
    max: Vector,

    query_x_range: Range<i32>,
    query_y_range: Range<i32>,

    // v -> [min + v, max'] where max' <= max
    rects: Vec<SparseT2Rect>,
}

impl SparseT2Query {
    pub fn new(min: Vector, max: Vector, query_height: usize, query_width: usize) -> Self {
        assert!(min.x <= max.x);
        assert!(min.y <= max.y);
        assert!(query_height % 4 == 0);
        assert!(query_width % 4 == 0);

        let query_x_range = min.x..(max.x - query_height as i32 + 2);
        let query_y_range = min.y..(max.y - query_width as i32 + 2);

        let mut rects = vec![];
        for x1 in min.x..min.x + 4 {
            for y1 in min.y..min.y + 4 {
                debug_assert!(max.x >= x1);
                debug_assert!(max.y >= y1);

                let x = max.x - x1 + 4 >> 2 << 2;
                let y = max.y - y1 + 4 >> 2 << 2;
                let rect = SparseT2Rect::new(x as usize, y as usize);

                rects.push(rect);
            }
        }

        Self {
            min,
            max,
            rects,
            query_x_range,
            query_y_range,
        }
    }

    pub fn flip(&mut self, v: &Vector) {
        debug_assert!((self.min.x..=self.max.x).contains(&(v.x as i32)));
        debug_assert!((self.min.y..=self.max.y).contains(&(v.y as i32)));

        let vx = (v.x as isize - self.min.x as isize) as usize;
        let vy = (v.y as isize - self.min.y as isize) as usize;

        for dx in 0..4 {
            for dy in 0..4 {
                if vx >= dx && vy >= dy {
                    self.rects[dx << 2 | dy].flip(Vector::new(vx - dx, vy - dy));
                }
            }
        }
    }

    pub fn query<'a>(
        &'a self,
        origin: Vector,
        mask: &'a SparseT2Rect,
    ) -> impl Iterator<Item = Vector> + 'a {
        debug_assert!(self.query_x_range.contains(&origin.x));
        debug_assert!(self.query_y_range.contains(&origin.y));

        let d = &origin - &self.min;
        let delta = Vector::new(d.x & 3, d.y & 3);
        let off = &self.min + &delta;

        let rel_origin = Vector::new((d.x as usize) >> 2 << 2, (d.y as usize) >> 2 << 2);

        self.rects[(delta.x << 2 | delta.y) as usize]
            .and_iter(rel_origin, mask)
            .map(move |v| &v + &off)
    }
}

#[cfg(test)]
mod tests {
    use crate::data::{sparse_t2_rect::SparseT2Rect, vector::Vector};

    use super::SparseT2Query;

    #[test]
    fn test_query() {
        let mut stq = SparseT2Query::new(
            super::Vector::new(-7, -15),
            super::Vector::new(10, 20),
            8,
            16,
        );

        stq.flip(&Vector::new(0, 0));
        stq.flip(&Vector::new(0, -10));
        stq.flip(&Vector::new(10, 20));

        let mut mask = SparseT2Rect::new(8, 16);
        mask.flip(Vector::new(0, 0));
        mask.flip(Vector::new(0, 1));
        mask.flip(Vector::new(0, 2));
        mask.flip(Vector::new(0, 10));
        mask.flip(Vector::new(1, 0));
        mask.flip(Vector::new(7, 15));

        assert_eq!(
            stq.query((0, 0).into(), &mask).collect::<Vec<_>>(),
            vec![(0, 0).into()]
        );
        assert_eq!(
            stq.query((0, -10).into(), &mask).collect::<Vec<_>>(),
            vec![(0, -10).into(), (0, 0).into()]
        );
        assert_eq!(
            stq.query((-7, -15).into(), &mask).collect::<Vec<_>>(),
            vec![(0, 0).into()]
        );
        assert_eq!(
            stq.query((3, 5).into(), &mask).collect::<Vec<_>>(),
            vec![(10, 20).into()]
        );
    }
}

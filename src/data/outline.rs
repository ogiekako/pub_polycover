use super::d4::D4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Outline {
    pub height: usize,
    pub width: usize,
}

impl From<(usize, usize)> for Outline {
    fn from((height, width): (usize, usize)) -> Self {
        Self { height, width }
    }
}

impl Outline {
    pub fn new(height: usize, width: usize) -> Self {
        Self { height, width }
    }

    pub fn cell(&self, x: usize, y: usize) -> OutlinedCell<'_> {
        OutlinedCell::new(self, x, y)
    }

    pub fn transform(&self, g: D4) -> Self {
        if g.get_rot() & 1 == 1 {
            Self::new(self.width, self.height)
        } else {
            *self
        }
    }
}

pub struct OutlinedCell<'a> {
    outline: &'a Outline,
    x: usize,
    y: usize,
}

impl<'a> OutlinedCell<'a> {
    fn new(outline: &'a Outline, x: usize, y: usize) -> Self {
        Self { outline, x, y }
    }

    pub fn transform(&self, g: D4) -> (usize, usize) {
        let (mut x, mut y) = (self.x, self.y);
        if MAGIC1 >> (g as usize) & 1 == 0 {
            x = self.outline.height - 1 - x;
        }
        if MAGIC2 >> (g as usize) & 1 == 0 {
            y = self.outline.width - 1 - y;
        }

        if (g as usize) & 1 != 0 {
            std::mem::swap(&mut x, &mut y);
        }

        (x, y)
    }
}

const MAGIC1: usize = 0b11000011;
const MAGIC2: usize = 0b10011001;

#[cfg(test)]
mod tests {
    use crate::data::d4::D4;

    use super::Outline;

    #[test]
    fn test_transform() {
        let outline = Outline::new(10, 20);
        let zero = outline.cell(0, 0);

        assert_eq!(zero.transform(D4::I), (0, 0));
        assert_eq!(zero.transform(D4::R1), (19, 0));
        assert_eq!(zero.transform(D4::R2), (9, 19));
        assert_eq!(zero.transform(D4::R3), (0, 9));
        assert_eq!(zero.transform(D4::S0), (9, 0));
        assert_eq!(zero.transform(D4::S1), (19, 9));
        assert_eq!(zero.transform(D4::S2), (0, 19));
        assert_eq!(zero.transform(D4::S3), (0, 0));

        let cell = outline.cell(3, 5);

        assert_eq!(cell.transform(D4::I), (3, 5));
        assert_eq!(cell.transform(D4::R1), (14, 3));
        assert_eq!(cell.transform(D4::R2), (6, 14));
        assert_eq!(cell.transform(D4::R3), (5, 6));
        assert_eq!(cell.transform(D4::S0), (6, 5));
        assert_eq!(cell.transform(D4::S1), (14, 6));
        assert_eq!(cell.transform(D4::S2), (3, 14));
        assert_eq!(cell.transform(D4::S3), (5, 3));

        for g1 in D4::all() {
            for g2 in D4::all() {
                let g = g1 * g2;

                let got = cell.transform(g);

                let c = cell.transform(g2);

                let want = outline.transform(g2).cell(c.0, c.1).transform(g1);

                assert_eq!(got, want, "{g1:?} {g2:?} {g:?}");
            }
        }
    }
}

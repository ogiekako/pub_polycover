use super::vector::Vector;

// https://en.wikipedia.org/wiki/Examples_of_groups#dihedral_group_of_order_8
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum D4 {
    I = 0,
    R1 = 1,
    R2 = 2,
    R3 = 3,
    S0 = 4,
    S1 = 5,
    S2 = 6,
    S3 = 7,
}

pub const ALL_D4: [D4; 8] = [
    D4::I,
    D4::R1,
    D4::R2,
    D4::R3,
    D4::S0,
    D4::S1,
    D4::S2,
    D4::S3,
];

pub const FLIPPED: [D4; 8] = [
    D4::S0,
    D4::S1,
    D4::S2,
    D4::S3,
    D4::I,
    D4::R1,
    D4::R2,
    D4::R3,
];

const TABLE: [[D4; 8]; 8] = {
    const fn m(x: usize, y: usize) -> D4 {
        ALL_D4[x].apply_inner(ALL_D4[y])
    }
    [
        [
            m(0, 0),
            m(0, 1),
            m(0, 2),
            m(0, 3),
            m(0, 4),
            m(0, 5),
            m(0, 6),
            m(0, 7),
        ],
        [
            m(1, 0),
            m(1, 1),
            m(1, 2),
            m(1, 3),
            m(1, 4),
            m(1, 5),
            m(1, 6),
            m(1, 7),
        ],
        [
            m(2, 0),
            m(2, 1),
            m(2, 2),
            m(2, 3),
            m(2, 4),
            m(2, 5),
            m(2, 6),
            m(2, 7),
        ],
        [
            m(3, 0),
            m(3, 1),
            m(3, 2),
            m(3, 3),
            m(3, 4),
            m(3, 5),
            m(3, 6),
            m(3, 7),
        ],
        [
            m(4, 0),
            m(4, 1),
            m(4, 2),
            m(4, 3),
            m(4, 4),
            m(4, 5),
            m(4, 6),
            m(4, 7),
        ],
        [
            m(5, 0),
            m(5, 1),
            m(5, 2),
            m(5, 3),
            m(5, 4),
            m(5, 5),
            m(5, 6),
            m(5, 7),
        ],
        [
            m(6, 0),
            m(6, 1),
            m(6, 2),
            m(6, 3),
            m(6, 4),
            m(6, 5),
            m(6, 6),
            m(6, 7),
        ],
        [
            m(7, 0),
            m(7, 1),
            m(7, 2),
            m(7, 3),
            m(7, 4),
            m(7, 5),
            m(7, 6),
            m(7, 7),
        ],
    ]
};

impl D4 {
    pub const fn new(flip: bool, rot: usize) -> Self {
        ALL_D4[(flip as usize) << 2 | rot & 3]
    }

    pub const fn get_rot(self) -> usize {
        self as usize & 3
    }

    pub const fn get_flip(self) -> bool {
        self as usize >= 4
    }

    pub fn roted(self, rot: usize) -> Self {
        let x = self as usize;
        ALL_D4[(x + rot) & 3 | x & 4]
    }

    pub fn flipped(self) -> Self {
        FLIPPED[self as usize]
    }

    const fn apply_inner(self, rhs: Self) -> Self {
        let flip = self.get_flip() ^ rhs.get_flip();
        let rot = if self.get_flip() {
            self.get_rot() + 4 - rhs.get_rot()
        } else {
            self.get_rot() + rhs.get_rot()
        };
        Self::new(flip, rot)
    }

    pub(crate) fn all() -> impl Iterator<Item = Self> {
        ALL_D4.iter().copied()
    }

    pub fn inverse(self) -> Self {
        if self == D4::R1 {
            D4::R3
        } else if self == D4::R3 {
            D4::R1
        } else {
            self
        }
    }
}

impl std::ops::Mul<D4> for D4 {
    type Output = D4;

    fn mul(self, rhs: D4) -> Self::Output {
        TABLE[self as usize][rhs as usize]
    }
}

impl std::ops::MulAssign for D4 {
    fn mul_assign(&mut self, rhs: D4) {
        *self = TABLE[*self as usize][rhs as usize]
    }
}

impl From<usize> for D4 {
    fn from(value: usize) -> Self {
        ALL_D4[value]
    }
}

impl std::ops::Mul<Vector> for D4 {
    type Output = Vector;

    fn mul(self, mut rhs: Vector) -> Self::Output {
        if (self as usize) & 1 != 0 {
            std::mem::swap(&mut rhs.x, &mut rhs.y);
        }

        if MAGIC1 >> (self as usize) & 1 == 0 {
            rhs.x = -rhs.x;
        }
        if MAGIC2 >> (self as usize) & 1 == 0 {
            rhs.y = -rhs.y;
        }

        rhs
    }
}

impl std::ops::Mul<&Vector> for &D4 {
    type Output = Vector;

    fn mul(self, rhs: &Vector) -> Self::Output {
        *self * *rhs
    }
}

const MAGIC1: usize = 0b11001001;
const MAGIC2: usize = 0b10010011;

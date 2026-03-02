use std::rc::Rc;

use anyhow::bail;

use crate::data::tight_poly::TightPoly;

use super::placement::Placement;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub cover_with_1_count: u32,
    pub cover_with_2_count: u32,
}

pub fn full_check(problem: &TightPoly, answer: &TightPoly) -> anyhow::Result<()> {
    if answer.cells().is_empty() {
        bail!("The answer is empty");
    }
    if answer.has_hole() {
        bail!("The answer has a hole");
    }
    if !answer.is_connected() {
        bail!("The answer is not connected");
    }
    let check_res = check(problem, answer);
    if check_res.cover_with_1_count > 0 {
        bail!("The problem is contained in the answer");
    }
    if check_res.cover_with_2_count > 0 {
        bail!(
            "The problem can be covered with two copies of the answer: count {}",
            check_res.cover_with_2_count
        );
    }

    let placements = mask_to_placements(problem, answer);
    if full_check_dfs(&placements, placements.len() as u32 - 1, &mut vec![]) {
        bail!("The problem can be covered with more than two copies of the answer");
    }
    Ok(())
}

fn full_check_dfs(
    placements: &Vec<Vec<Placement>>,
    remaining: u32,
    placed: &mut Vec<Placement>,
) -> bool {
    if remaining == 0 {
        return true;
    }

    let mut mask = remaining;
    let last_bit = 1 << remaining.trailing_zeros();

    while mask > 0 {
        if mask & last_bit > 0 {
            for p in placements[mask as usize].iter() {
                if placed.iter().any(|p2| p.intersects(p2)) {
                    continue;
                }

                placed.push(p.clone());
                if full_check_dfs(placements, remaining ^ mask, placed) {
                    return true;
                }
                placed.pop();
            }
        }

        mask = (mask - 1) & remaining;
    }

    false
}

pub fn check(problem: &TightPoly, answer: &TightPoly) -> CheckResult {
    let placements = mask_to_placements(problem, answer);

    let cover_with_1_count = placements[placements.len() - 1].len() as u32;

    if cover_with_1_count > 0 {
        return CheckResult {
            cover_with_1_count,
            cover_with_2_count: 0,
        };
    }

    let mut cover_with_2_count = 0;

    let complete_mask = placements.len() - 1;

    for mask1 in (1..complete_mask).step_by(2) {
        let mask2 = complete_mask ^ mask1;

        for p1 in placements[mask1].iter() {
            for p2 in placements[mask2].iter() {
                if !p1.intersects(p2) {
                    cover_with_2_count += 1;
                }
            }
        }
    }

    CheckResult {
        cover_with_1_count,
        cover_with_2_count,
    }
}

fn mask_to_placements(problem: &TightPoly, answer: &TightPoly) -> Vec<Vec<Placement>> {
    let cands = answer.unique_rot_revs();

    let problem_cells = problem.cells();

    let mut res = vec![vec![]; 1 << problem_cells.len()];

    for cand in cands.into_iter() {
        let mut directions = vec![];

        for cell in cand.cells() {
            for problem_cell in problem_cells.iter() {
                let dir = (-cell.x + problem_cell.x, -cell.y + problem_cell.y);
                directions.push(dir);
            }
        }

        directions.sort();
        directions.dedup();

        let cand = Rc::new(cand);

        for (dx, dy) in directions {
            let placement = Placement::new(dx, dy, cand.clone());

            let mut mask = 0;
            for (i, cell) in problem_cells.iter().enumerate() {
                if placement.get(cell.x as u32, cell.y as u32) {
                    mask |= 1 << i;
                }
            }

            res[mask].push(placement);
        }
    }
    res
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::{check::check::full_check, data::tight_poly::TightPoly};

    use super::{check, mask_to_placements};

    #[test]
    fn test_mask_to_placements() {
        let problem = TightPoly::from_str(
            r#"
2 2
##
#.
"#,
        )
        .unwrap();
        let answer = TightPoly::from_str(
            r#"
1 2
##
"#,
        )
        .unwrap();

        let res = mask_to_placements(&problem, &answer);

        assert_eq!(res[0b000].len(), 0);
        assert_eq!(res[0b001].len(), 2);
        assert_eq!(res[0b010].len(), 3);
        assert_eq!(res[0b100].len(), 3);
        assert_eq!(res[0b011].len(), 1);
        assert_eq!(res[0b101].len(), 1);
        assert_eq!(res[0b110].len(), 0);
        assert_eq!(res[0b111].len(), 0);
    }

    #[test]
    fn test_check() {
        let problem = TightPoly::from_str(
            r#"
2 2
##
#.
"#,
        )
        .unwrap();
        let answer = TightPoly::from_str(
            r#"
1 2
##
"#,
        )
        .unwrap();

        let res = check(&problem, &answer);

        assert_eq!(
            res,
            super::CheckResult {
                cover_with_1_count: 0,
                cover_with_2_count: 6,
            }
        );
    }

    #[test]
    fn test_full_check() {
        for (problem, answer, want_err) in [
            (
                r#"
2 2
##
#.
"#,
                r#"
1 1
#
"#,
                "The problem can be covered with more than two copies of the answer",
            ),
            (
                r#"
2 2
##
#.
"#,
                r#"
3 3
###
#.#
##.
"#,
                "The answer has a hole",
            ),
            (
                r#"
2 2
##
#.
"#,
                r#"
3 3
###
..#
.#.
"#,
                "The answer is not connected",
            ),
            (
                r#"
2 2
##
#.
"#,
                r#"
2 3
###
..#
"#,
                "The problem is contained in the answer",
            ),
            (
                r#"
2 2
##
#.
"#,
                r#"
1 2
##
"#,
                "The problem can be covered with two copies of the answer",
            ),
            (
                r#"
3 3
###
###
##.
"#,
                r#"
4 5
.#.#
##.##
#...#
#####
"#,
                "The problem can be covered with more than two copies of the answer",
            ),
            (
                r#"
3 3
###
###
#.#
"#,
                r#"
4 5
.#.#
##.##
#...#
#####
"#,
                "The problem can be covered with two copies of the answer",
            ),
        ] {
            let problem = TightPoly::from_str(problem).unwrap();
            let answer = TightPoly::from_str(answer).unwrap();

            let res = full_check(&problem, &answer).unwrap_err().to_string();

            assert!(res.contains(want_err));
        }
    }

    #[test]
    fn test_full_check_pass() {
        let problem = TightPoly::from_str(
            r#"
3 3
###
###
###
"#,
        )
        .unwrap();
        let answer = TightPoly::from_str(
            r#"
4 5
.#.#.
##.##
#...#
#####
"#,
        )
        .unwrap();

        full_check(&problem, &answer).unwrap();
    }
}

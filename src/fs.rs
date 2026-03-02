use std::{path::PathBuf, str::FromStr};

use anyhow::Context;
use log::info;

use crate::{check::check::full_check, data::tight_poly::TightPoly};

#[derive(Debug, Clone)]
pub struct ParsedProblem {
    pub cell_count: usize,
    pub name: String,
    pub solved: bool,
    pub problem: TightPoly,
}

impl ParsedProblem {
    fn solution_name(&self) -> String {
        format!("{n}/{name}.txt", n = self.cell_count, name = self.name)
    }

    pub fn problem_name(&self) -> String {
        format!(
            "{n}/{name}.{ext}",
            n = self.cell_count,
            name = self.name,
            ext = if self.solved { "yes" } else { "never" }
        )
    }
}

pub struct Client {
    problem_dir: PathBuf,
    solution_dir: PathBuf,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    pub fn new() -> Self {
        Self {
            problem_dir: PathBuf::from("problem"),
            solution_dir: PathBuf::from("solution"),
        }
    }

    pub fn read_problems(&self) -> anyhow::Result<Vec<ParsedProblem>> {
        let mut res = vec![];
        for n in 5..=7 {
            let dir = self.problem_dir.join(format!("{}", n));
            for entry in dir.read_dir()? {
                res.push(self.parse_problem(entry?.path())?);
            }
        }
        Ok(res)
    }

    pub fn parse_problem(&self, path: PathBuf) -> anyhow::Result<ParsedProblem> {
        let filename = path
            .file_name()
            .context("path has no filename")?
            .to_str()
            .context("path is not utf8")?
            .to_string();
        let name_stem = filename.split('.').collect::<Vec<_>>();

        let name = name_stem[0];
        let solved = name_stem[1] == "yes";

        let cell_count = path
            .parent()
            .context("path has no parent")?
            .file_name()
            .context("path's parent has no filename")?
            .to_str()
            .context("path's parent is not utf8")?
            .parse::<usize>()?;

        Ok(ParsedProblem {
            cell_count,
            name: name.to_string(),
            solved,
            problem: TightPoly::from_str(&std::fs::read_to_string(path)?)?,
        })
    }

    pub fn read_solution(&self, problem: &ParsedProblem) -> anyhow::Result<TightPoly> {
        let filename = format!(
            "{n}/{name}.txt",
            n = problem.cell_count,
            name = problem.name
        );
        let path = self.solution_dir.join(filename);
        let solution = TightPoly::from_str(&std::fs::read_to_string(path)?)?;
        full_check(&problem.problem, &solution)?;
        Ok(solution)
    }

    fn problem_file_name(&self, problem: &ParsedProblem) -> PathBuf {
        self.problem_dir.join(problem.problem_name())
    }

    fn solution_file_name(&self, problem: &ParsedProblem) -> PathBuf {
        self.solution_dir.join(problem.solution_name())
    }

    pub fn write_solution_if_better(
        &self,
        problem: &ParsedProblem,
        solution: TightPoly,
    ) -> anyhow::Result<bool> {
        full_check(&problem.problem, &solution)?;

        let name = problem.problem_name();
        if problem.solved {
            let current = self.read_solution(problem)?;

            let size_ord =
                (current.height() * current.width()).cmp(&(solution.height() * solution.width()));
            let count_ord = (current.cells().len()).cmp(&solution.cells().len());
            let asymm_ord = (current.asymmetricity())
                .partial_cmp(&solution.asymmetricity())
                .unwrap();

            if size_ord.then(count_ord).then(asymm_ord) != std::cmp::Ordering::Greater {
                info!("{name}: not a better solution");
                return Ok(false);
            }
        }

        let filename = format!(
            "{n}/{name}.txt",
            n = problem.cell_count,
            name = problem.name
        );
        let path = self.solution_dir.join(filename);

        std::fs::write(path, format!("{}", solution))?;

        let mut solved_problem = problem.clone();

        if !problem.solved {
            solved_problem.solved = true;
            let _ = std::fs::rename(
                self.problem_file_name(problem),
                self.problem_file_name(&solved_problem),
            );
        }

        info!("{name}: better solution has been written");

        Ok(true)
    }

    pub fn remove_problem_and_solution(&self, problem: &ParsedProblem) -> anyhow::Result<()> {
        let sol = self.solution_file_name(problem);
        let prob = self.problem_file_name(problem);

        std::fs::remove_file(prob)?;
        std::fs::remove_file(sol)?;

        info!(
            "Removed {} and {}",
            problem.problem_name(),
            problem.solution_name()
        );

        Ok(())
    }
}

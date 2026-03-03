use rayon::prelude::*;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const N: i32 = 10;
const CELLS: usize = (N as usize) * (N as usize);

type Mask = u128;
type Pt = (i32, i32);

#[derive(Clone, Debug, Serialize)]
struct Found {
    seed_name: String,
    flips: usize,
    size: usize,
    holes: usize,
    rows: Vec<String>,
}

#[derive(Clone)]
struct Context {
    all_or: Vec<Vec<Pt>>, // normalized isometries
    regions: Vec<HashSet<Pt>>, // Q0, P1, P2, P3
    size: usize,
    holes: usize,
    rows: Vec<String>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Tri {
    Yes,
    No,
    Unknown,
}

fn idx(x: i32, y: i32) -> usize {
    (y * N + x) as usize
}
fn bit(x: i32, y: i32) -> Mask {
    1u128 << idx(x, y)
}

fn parse_rows(rows: &[&str]) -> Mask {
    let h = rows.len() as i32;
    let mut m = 0u128;
    for (r, row) in rows.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            if ch == '1' {
                let x = c as i32;
                let y = h - 1 - r as i32;
                m |= bit(x, y);
            }
        }
    }
    m
}

fn coords_from_mask(m: Mask) -> Vec<Pt> {
    let mut v = Vec::new();
    for y in 0..N {
        for x in 0..N {
            if (m & bit(x, y)) != 0 {
                v.push((x, y));
            }
        }
    }
    v
}

fn rows_from_mask(m: Mask) -> Vec<String> {
    let mut rows = Vec::new();
    for y in (0..N).rev() {
        let mut s = String::new();
        for x in 0..N {
            s.push(if (m & bit(x, y)) != 0 { '1' } else { '0' });
        }
        rows.push(s);
    }
    rows
}

fn rotate90_fixed(m: Mask) -> Mask {
    let mut out = 0u128;
    for (x, y) in coords_from_mask(m) {
        out |= bit(N - 1 - y, x);
    }
    out
}

fn reflect_x_fixed(m: Mask) -> Mask {
    let mut out = 0u128;
    for (x, y) in coords_from_mask(m) {
        out |= bit(N - 1 - x, y);
    }
    out
}

fn bbox_coords(coords: &[Pt]) -> Option<(i32, i32, i32, i32)> {
    if coords.is_empty() {
        return None;
    }
    let mut minx = i32::MAX;
    let mut maxx = i32::MIN;
    let mut miny = i32::MAX;
    let mut maxy = i32::MIN;
    for &(x, y) in coords {
        minx = minx.min(x);
        maxx = maxx.max(x);
        miny = miny.min(y);
        maxy = maxy.max(y);
    }
    Some((minx, maxx, miny, maxy))
}

fn bbox_mask(m: Mask) -> Option<(i32, i32, i32, i32)> {
    bbox_coords(&coords_from_mask(m))
}

fn normalize_coords(coords: &[Pt]) -> Vec<Pt> {
    let (minx, _, miny, _) = bbox_coords(coords).unwrap();
    coords.iter().map(|&(x, y)| (x - minx, y - miny)).collect()
}

fn set_from_coords(coords: &[Pt]) -> HashSet<Pt> {
    coords.iter().cloned().collect()
}

fn connected_set(set: &HashSet<Pt>) -> bool {
    if set.is_empty() {
        return false;
    }
    let start = *set.iter().next().unwrap();
    let mut q = VecDeque::new();
    let mut vis = HashSet::new();
    q.push_back(start);
    vis.insert(start);
    while let Some((x, y)) = q.pop_front() {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let np = (x + dx, y + dy);
            if set.contains(&np) && !vis.contains(&np) {
                vis.insert(np);
                q.push_back(np);
            }
        }
    }
    vis.len() == set.len()
}

fn connected_mask(m: Mask) -> bool {
    connected_set(&set_from_coords(&coords_from_mask(m)))
}

fn hole_count_mask(m: Mask) -> usize {
    let coords = coords_from_mask(m);
    if coords.is_empty() {
        return 0;
    }
    let (minx0, maxx0, miny0, maxy0) = bbox_coords(&coords).unwrap();
    let minx = minx0 - 1;
    let maxx = maxx0 + 1;
    let miny = miny0 - 1;
    let maxy = maxy0 + 1;
    let filled = set_from_coords(&coords);
    let mut q = VecDeque::new();
    let mut vis = HashSet::new();
    q.push_back((minx, miny));
    vis.insert((minx, miny));
    while let Some((x, y)) = q.pop_front() {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let nx = x + dx;
            let ny = y + dy;
            if nx < minx || nx > maxx || ny < miny || ny > maxy {
                continue;
            }
            let p = (nx, ny);
            if filled.contains(&p) || vis.contains(&p) {
                continue;
            }
            vis.insert(p);
            q.push_back(p);
        }
    }
    let mut holes = 0;
    for x in minx..=maxx {
        for y in miny..=maxy {
            let p = (x, y);
            if !filled.contains(&p) && !vis.contains(&p) {
                holes += 1;
            }
        }
    }
    holes
}

fn unique_isometries_norm(base_fixed: Mask) -> Vec<Vec<Pt>> {
    let mut out: Vec<Vec<Pt>> = Vec::new();
    let mut seen: Vec<HashSet<Pt>> = Vec::new();
    let mut cur = base_fixed;
    for _ in 0..4 {
        let r = normalize_coords(&coords_from_mask(cur));
        let rs = set_from_coords(&r);
        if !seen.iter().any(|s| *s == rs) {
            seen.push(rs);
            out.push(r);
        }
        let rf = normalize_coords(&coords_from_mask(reflect_x_fixed(cur)));
        let rfs = set_from_coords(&rf);
        if !seen.iter().any(|s| *s == rfs) {
            seen.push(rfs);
            out.push(rf);
        }
        cur = rotate90_fixed(cur);
    }
    out
}

fn single_covers(orients: &[Vec<Pt>], region: &HashSet<Pt>) -> HashSet<(usize, i32, i32)> {
    let mut ans = HashSet::new();
    let rvec: Vec<Pt> = region.iter().cloned().collect();
    let (rminx, rmaxx, rminy, rmaxy) = bbox_coords(&rvec).unwrap();

    for (oi, s) in orients.iter().enumerate() {
        let (sminx, smaxx, sminy, smaxy) = bbox_coords(s).unwrap();
        let sset = set_from_coords(s);
        for tx in (rminx - smaxx)..=(rmaxx - sminx) {
            for ty in (rminy - smaxy)..=(rmaxy - sminy) {
                let mut ok = true;
                for &(x, y) in &rvec {
                    if !sset.contains(&(x - tx, y - ty)) {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    ans.insert((oi, tx, ty));
                }
            }
        }
    }
    ans
}

#[derive(Clone)]
struct Placement {
    cells: Vec<Pt>,
    cover: u128,
}

fn build_placements(orients: &[Vec<Pt>], region: &HashSet<Pt>) -> (Vec<Placement>, Vec<Vec<usize>>, u128) {
    let rvec: Vec<Pt> = region.iter().cloned().collect();
    let mut ridx: HashMap<Pt, usize> = HashMap::new();
    for (i, p) in rvec.iter().enumerate() {
        ridx.insert(*p, i);
    }
    let full = if rvec.len() == 128 {
        u128::MAX
    } else {
        (1u128 << rvec.len()) - 1
    };

    let (rminx, rmaxx, rminy, rmaxy) = bbox_coords(&rvec).unwrap();
    let mut placements = Vec::new();

    for s in orients {
        let (sminx, smaxx, sminy, smaxy) = bbox_coords(s).unwrap();
        for tx in (rminx - smaxx)..=(rmaxx - sminx) {
            for ty in (rminy - smaxy)..=(rmaxy - sminy) {
                let cells: Vec<Pt> = s.iter().map(|&(x, y)| (x + tx, y + ty)).collect();
                let mut cover = 0u128;
                for &(x, y) in &cells {
                    if let Some(&i) = ridx.get(&(x, y)) {
                        cover |= 1u128 << i;
                    }
                }
                if cover != 0 {
                    placements.push(Placement { cells, cover });
                }
            }
        }
    }

    let mut by_cell = vec![Vec::<usize>::new(); rvec.len()];
    for (pi, p) in placements.iter().enumerate() {
        let mut mm = p.cover;
        while mm != 0 {
            let lb = mm & (!mm + 1);
            let bi = lb.trailing_zeros() as usize;
            by_cell[bi].push(pi);
            mm ^= lb;
        }
    }

    (placements, by_cell, full)
}

fn has_multi(orients: &[Vec<Pt>], region: &HashSet<Pt>, node_limit: usize, max_depth: usize) -> Tri {
    let (placements, by_cell, full) = build_placements(orients, region);

    fn dfs(
        cov: u128,
        occupied: &mut HashSet<Pt>,
        depth: usize,
        placements: &[Placement],
        by_cell: &[Vec<usize>],
        full: u128,
        nodes: &mut usize,
        node_limit: usize,
        max_depth: usize,
    ) -> Tri {
        *nodes += 1;
        if *nodes > node_limit {
            return Tri::Unknown;
        }
        if cov == full {
            return if depth >= 2 { Tri::Yes } else { Tri::No };
        }
        if depth >= max_depth {
            return Tri::No;
        }

        let mut uncovered = full ^ cov;
        let mut best_cell: Option<usize> = None;
        let mut best_opts: Vec<usize> = Vec::new();

        while uncovered != 0 {
            let lb = uncovered & (!uncovered + 1);
            let cell = lb.trailing_zeros() as usize;
            uncovered ^= lb;

            let mut opts = Vec::new();
            for &pi in &by_cell[cell] {
                let p = &placements[pi];
                if p.cells.iter().any(|c| occupied.contains(c)) {
                    continue;
                }
                opts.push(pi);
            }
            if opts.is_empty() {
                return Tri::No;
            }
            if best_cell.is_none() || opts.len() < best_opts.len() {
                best_cell = Some(cell);
                best_opts = opts;
                if best_opts.len() == 1 {
                    break;
                }
            }
        }

        let un = full ^ cov;
        best_opts.sort_by_key(|&pi| {
            let gain = (placements[pi].cover & un).count_ones() as i32;
            -gain
        });

        for pi in best_opts {
            let p = &placements[pi];
            let mut added = Vec::new();
            for &c in &p.cells {
                if occupied.insert(c) {
                    added.push(c);
                }
            }

            let r = dfs(
                cov | p.cover,
                occupied,
                depth + 1,
                placements,
                by_cell,
                full,
                nodes,
                node_limit,
                max_depth,
            );

            for c in added {
                occupied.remove(&c);
            }

            match r {
                Tri::Yes => return Tri::Yes,
                Tri::Unknown => return Tri::Unknown,
                Tri::No => {}
            }
        }

        Tri::No
    }

    let mut nodes = 0usize;
    dfs(
        0,
        &mut HashSet::new(),
        0,
        &placements,
        &by_cell,
        full,
        &mut nodes,
        node_limit,
        max_depth,
    )
}

fn build_context(mask: Mask) -> Option<Context> {
    if !connected_mask(mask) {
        return None;
    }
    if bbox_mask(mask)? != (0, N - 1, 0, N - 1) {
        return None;
    }

    let r0 = mask;
    let r1 = rotate90_fixed(mask);
    let r3 = rotate90_fixed(rotate90_fixed(rotate90_fixed(mask)));

    // distinct colors
    if r0 == r1 || r0 == r3 || r1 == r3 {
        return None;
    }

    let q_fixed = r0 & r1 & r3;
    let qf = coords_from_mask(q_fixed);
    let (qminx, qmaxx, qminy, qmaxy) = bbox_coords(&qf)?;
    if (qmaxx - qminx + 1, qmaxy - qminy + 1) != (8, 8) {
        return None;
    }

    let q_norm: Vec<Pt> = qf.iter().map(|&(x, y)| (x - qminx, y - qminy)).collect();
    let q_set = set_from_coords(&q_norm);
    if !connected_set(&q_set) {
        return None;
    }

    for (dx, dy) in [(8, 0), (-8, 0), (0, 8), (0, -8)] {
        let shifted: HashSet<Pt> = q_set.iter().map(|&(x, y)| (x + dx, y + dy)).collect();
        let mut u = q_set.clone();
        u.extend(shifted.iter());
        if !connected_set(&u) {
            return None;
        }
    }

    let a = (-qminx, -qminy);

    let r0n = normalize_coords(&coords_from_mask(r0));
    let r1n = normalize_coords(&coords_from_mask(r1));
    let r3n = normalize_coords(&coords_from_mask(r3));

    let p1: HashSet<Pt> = r0n.iter().map(|&(x, y)| (x + a.0, y + a.1)).collect();
    let p2: HashSet<Pt> = r1n.iter().map(|&(x, y)| (x + a.0, y + a.1)).collect();
    let p3: HashSet<Pt> = r3n.iter().map(|&(x, y)| (x + a.0, y + a.1)).collect();

    for (dx, dy) in [(8, 0), (-8, 0), (0, 8), (0, -8)] {
        let regs = [&p1, &p2, &p3];
        for i in 0..3 {
            for j in 0..3 {
                let sj: HashSet<Pt> = regs[j].iter().map(|&(x, y)| (x + dx, y + dy)).collect();
                let ov = regs[i].iter().any(|p| sj.contains(p));
                if ov != (i == j) {
                    return None;
                }
            }
        }
    }

    let all_or = unique_isometries_norm(mask);

    let c0 = set_from_coords(&r0n);
    let c1 = set_from_coords(&r1n);
    let c2 = set_from_coords(&r3n);

    let mut idxs = Vec::new();
    for t in [&c0, &c1, &c2] {
        let mut found = None;
        for (i, o) in all_or.iter().enumerate() {
            if set_from_coords(o) == *t {
                found = Some(i);
                break;
            }
        }
        idxs.push(found?);
    }

    let exp_q: HashSet<(usize, i32, i32)> =
        HashSet::from([(idxs[0], a.0, a.1), (idxs[1], a.0, a.1), (idxs[2], a.0, a.1)]);
    if single_covers(&all_or, &q_set) != exp_q {
        return None;
    }

    let regs = [p1.clone(), p2.clone(), p3.clone()];
    for k in 0..3 {
        let exp = HashSet::from([(idxs[k], a.0, a.1)]);
        if single_covers(&all_or, &regs[k]) != exp {
            return None;
        }
    }

    Some(Context {
        all_or,
        regions: vec![q_set, p1, p2, p3],
        size: coords_from_mask(mask).len(),
        holes: hole_count_mask(mask),
        rows: rows_from_mask(mask),
    })
}

fn verify_multi(ctx: &Context, deep: bool) -> Tri {
    let (limit, depth) = if deep {
        (900_000usize, 18usize)
    } else {
        (80_000usize, 12usize)
    };

    for r in &ctx.regions {
        match has_multi(&ctx.all_or, r, limit, depth) {
            Tri::Yes => return Tri::Yes,
            Tri::Unknown => return Tri::Unknown,
            Tri::No => {}
        }
    }
    Tri::No
}

fn eval_mask(mask: Mask) -> Option<Found> {
    let ctx = build_context(mask)?;

    // quick multi first
    if verify_multi(&ctx, false) != Tri::No {
        return None;
    }

    // deep multi
    if verify_multi(&ctx, true) != Tri::No {
        return None;
    }

    Some(Found {
        seed_name: String::new(),
        flips: 0,
        size: ctx.size,
        holes: ctx.holes,
        rows: ctx.rows,
    })
}

fn k123(seed: Mask, seed_name: &str) -> Vec<Found> {
    let mut masks: Vec<(usize, Mask)> = Vec::new();
    masks.push((0, seed));
    for i in 0..CELLS {
        masks.push((1, seed ^ (1u128 << i)));
    }
    for i in 0..CELLS {
        for j in (i + 1)..CELLS {
            masks.push((2, seed ^ (1u128 << i) ^ (1u128 << j)));
        }
    }
    for i in 0..CELLS {
        for j in (i + 1)..CELLS {
            for k in (j + 1)..CELLS {
                masks.push((3, seed ^ (1u128 << i) ^ (1u128 << j) ^ (1u128 << k)));
            }
        }
    }

    masks
        .par_iter()
        .filter_map(|(f, m)| {
            let mut r = eval_mask(*m)?;
            r.flips = *f;
            r.seed_name = seed_name.to_string();
            Some(r)
        })
        .collect()
}

fn k4(seed: Mask, seed_name: &str) -> Vec<Found> {
    let found: Arc<Mutex<Vec<Found>>> = Arc::new(Mutex::new(Vec::new()));
    let tested = Arc::new(AtomicUsize::new(0));

    (0..(CELLS - 3)).into_par_iter().for_each(|i| {
        let mut local = Vec::new();
        for j in (i + 1)..(CELLS - 2) {
            for k in (j + 1)..(CELLS - 1) {
                for l in (k + 1)..CELLS {
                    let m = seed ^ (1u128 << i) ^ (1u128 << j) ^ (1u128 << k) ^ (1u128 << l);
                    if let Some(mut r) = eval_mask(m) {
                        r.flips = 4;
                        r.seed_name = seed_name.to_string();
                        local.push(r);
                    }
                    let c = tested.fetch_add(1, Ordering::Relaxed) + 1;
                    if c % 300_000 == 0 {
                        eprintln!("tested {} (k4, {})", c, seed_name);
                    }
                }
            }
        }
        if !local.is_empty() {
            let mut g = found.lock().unwrap();
            g.extend(local);
        }
    });

    Arc::try_unwrap(found).unwrap().into_inner().unwrap()
}

fn parse_max_flips_arg() -> usize {
    let args: Vec<String> = env::args().collect();
    let mut max_flips = 4usize;
    let mut i = 1usize;

    while i < args.len() {
        match args[i].as_str() {
            "--max-flips" => {
                if i + 1 >= args.len() {
                    panic!("--max-flips requires a value");
                }
                max_flips = args[i + 1]
                    .parse::<usize>()
                    .expect("failed to parse --max-flips as usize");
                i += 2;
            }
            "-h" | "--help" => {
                println!("Usage: gadget_rayon_search [--max-flips K]");
                println!("  --max-flips K   Search up to Hamming distance K from seeds (default: 4)");
                std::process::exit(0);
            }
            other => {
                panic!("unknown argument: {}", other);
            }
        }
    }

    if max_flips > 4 {
        eprintln!("warning: max-flips={} requested, but implementation supports up to 4; using 4", max_flips);
        4
    } else {
        max_flips
    }
}

fn main() {
    let max_flips = parse_max_flips_arg();
    eprintln!("running with max-flips={}", max_flips);

    let old_rows = [
        "0000001000",
        "0110001110",
        "0111111110",
        "0011111100",
        "0011111100",
        "1111111111",
        "0011111100",
        "0111111110",
        "0100001110",
        "0000001000",
    ];

    let near_rows = [
        "0000001000",
        "0110001110",
        "0111011110",
        "0011111100",
        "0011110100",
        "1111111111",
        "0011111100",
        "0110111110",
        "0100001110",
        "0000001000",
    ];

    let seeds = vec![
        ("old", parse_rows(&old_rows)),
        ("near", parse_rows(&near_rows)),
    ];

    let mut all_found = Vec::<Found>::new();

    for (name, seed) in seeds {
        eprintln!("search seed {}: k<=3", name);
        let mut f = k123(seed, name);
        f.retain(|x| x.flips <= max_flips);
        eprintln!("seed {} k<=3 found {}", name, f.len());
        all_found.append(&mut f);

        if max_flips >= 4 {
            eprintln!("search seed {}: k=4", name);
            let mut f4 = k4(seed, name);
            eprintln!("seed {} k=4 found {}", name, f4.len());
            all_found.append(&mut f4);
        }
    }

    // Tie-break by seed and rows so JSON output order is deterministic across runs.
    all_found.sort_by(|a, b| {
        (a.holes, a.flips, a.size, &a.seed_name)
            .cmp(&(b.holes, b.flips, b.size, &b.seed_name))
            .then_with(|| a.rows.cmp(&b.rows))
    });
    eprintln!("total found {}", all_found.len());
    println!("{}", serde_json::to_string_pretty(&all_found).unwrap());
}

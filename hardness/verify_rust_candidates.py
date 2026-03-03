#!/usr/bin/env python3
import argparse
import json
from collections import deque, defaultdict
from functools import lru_cache

CAND_PATH = '/tmp/rust_search_results.json'


def parse_rows(rows):
    n = len(rows)
    cells = set()
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            if ch == '1':
                cells.add((x, y))
    return cells, n


def bbox(cells):
    xs = [x for x, _ in cells]
    ys = [y for _, y in cells]
    return min(xs), max(xs), min(ys), max(ys)


def normalize(cells):
    mnx, _, mny, _ = bbox(cells)
    return {(x - mnx, y - mny) for x, y in cells}


def shift(cells, dx, dy):
    return {(x + dx, y + dy) for x, y in cells}


def connected(cells):
    if not cells:
        return False
    s = next(iter(cells))
    q = deque([s])
    vis = {s}
    while q:
        x, y = q.popleft()
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            p = (x + dx, y + dy)
            if p in cells and p not in vis:
                vis.add(p)
                q.append(p)
    return len(vis) == len(cells)


def rot90(cells, n):
    return {(n - 1 - y, x) for x, y in cells}


def rot180(cells, n):
    return {(n - 1 - x, n - 1 - y) for x, y in cells}


def rot270(cells, n):
    return {(y, n - 1 - x) for x, y in cells}


def reflx(cells, n):
    return {(n - 1 - x, y) for x, y in cells}


def unique_isometries_norm(cells, n):
    r0 = cells
    r1 = rot90(r0, n)
    r2 = rot180(r0, n)
    r3 = rot270(r0, n)
    f0 = reflx(r0, n)
    f1 = rot90(f0, n)
    f2 = rot180(f0, n)
    f3 = rot270(f0, n)
    out = []
    seen = set()
    for v in (r0, r1, r2, r3, f0, f1, f2, f3):
        vn = frozenset(normalize(v))
        if vn not in seen:
            seen.add(vn)
            out.append(set(vn))
    return out


def single_covers(orients, region):
    rlist = list(region)
    rminx, rmaxx, rminy, rmaxy = bbox(region)
    ans = set()
    for oi, s in enumerate(orients):
        sminx, smaxx, sminy, smaxy = bbox(s)
        for tx in range(rminx - smaxx, rmaxx - sminx + 1):
            for ty in range(rminy - smaxy, rmaxy - sminy + 1):
                ok = True
                for x, y in rlist:
                    if (x - tx, y - ty) not in s:
                        ok = False
                        break
                if ok:
                    ans.add((oi, tx, ty))
    return ans


def build_placements(orients, region):
    ridx = {c: i for i, c in enumerate(sorted(region))}
    full = (1 << len(ridx)) - 1

    rminx, rmaxx, rminy, rmaxy = bbox(region)
    uniq = {}
    for s in orients:
        sminx, smaxx, sminy, smaxy = bbox(s)
        for tx in range(rminx - smaxx, rmaxx - sminx + 1):
            for ty in range(rminy - smaxy, rmaxy - sminy + 1):
                cells = {(x + tx, y + ty) for x, y in s}
                cov = 0
                for c in cells:
                    i = ridx.get(c)
                    if i is not None:
                        cov |= 1 << i
                if cov:
                    uniq[frozenset(cells)] = (cells, cov)

    placements = list(uniq.values())
    by_cell = [[] for _ in range(len(ridx))]
    for pi, (_, cov) in enumerate(placements):
        mm = cov
        while mm:
            lb = mm & -mm
            bi = lb.bit_length() - 1
            by_cell[bi].append(pi)
            mm ^= lb
    return placements, by_cell, full


def has_multi(orients, region, node_limit=2_000_000, max_depth=20):
    placements, by_cell, full = build_placements(orients, region)
    nodes = 0

    @lru_cache(maxsize=None)
    def popcount(x):
        return x.bit_count()

    def dfs(cov, occupied, depth):
        nonlocal nodes
        nodes += 1
        if nodes > node_limit:
            return 'unknown'
        if cov == full:
            return 'yes' if depth >= 2 else 'no'
        if depth >= max_depth:
            return 'no'

        un = full ^ cov
        best = None
        best_opts = None

        mm = un
        while mm:
            lb = mm & -mm
            cell = lb.bit_length() - 1
            mm ^= lb
            opts = []
            for pi in by_cell[cell]:
                cells, pcov = placements[pi]
                if occupied & hash_cells_bits(cells):
                    continue
                opts.append(pi)
            if not opts:
                return 'no'
            if best_opts is None or len(opts) < len(best_opts):
                best = cell
                best_opts = opts
                if len(best_opts) == 1:
                    break

        best_opts.sort(key=lambda pi: -popcount(placements[pi][1] & un))
        for pi in best_opts:
            cells, pcov = placements[pi]
            bits = hash_cells_bits(cells)
            if bits & occupied:
                continue
            r = dfs(cov | pcov, occupied | bits, depth + 1)
            if r in ('yes', 'unknown'):
                return r
        return 'no'

    # coordinate compression for non-overlap bitset
    all_coords = sorted({c for cells, _ in placements for c in cells})
    cid = {c: i for i, c in enumerate(all_coords)}
    cell_bits = []
    for cells, cov in placements:
        b = 0
        for c in cells:
            b |= 1 << cid[c]
        cell_bits.append(b)

    def local_hash(cells):
        raise RuntimeError

    # monkey patch fast lookup through closure array index
    # rebuild placements tuple to carry bitset
    placements2 = []
    for i, (cells, cov) in enumerate(placements):
        placements2.append((cells, cov, cell_bits[i]))
    placements = placements2

    def hash_cells_bits(cells):
        # never called with arbitrary cells after rewriting; kept for structure parity
        return 0

    def dfs2(cov, occupied, depth):
        nonlocal nodes
        nodes += 1
        if nodes > node_limit:
            return 'unknown'
        if cov == full:
            return 'yes' if depth >= 2 else 'no'
        if depth >= max_depth:
            return 'no'

        un = full ^ cov
        best_opts = None
        mm = un
        while mm:
            lb = mm & -mm
            cell = lb.bit_length() - 1
            mm ^= lb
            opts = []
            for pi in by_cell[cell]:
                _, _, bits = placements[pi]
                if bits & occupied:
                    continue
                opts.append(pi)
            if not opts:
                return 'no'
            if best_opts is None or len(opts) < len(best_opts):
                best_opts = opts
                if len(best_opts) == 1:
                    break

        best_opts.sort(key=lambda pi: -popcount(placements[pi][1] & un))
        for pi in best_opts:
            _, pcov, bits = placements[pi]
            r = dfs2(cov | pcov, occupied | bits, depth + 1)
            if r in ('yes', 'unknown'):
                return r
        return 'no'

    st = dfs2(0, 0, 0)
    return st, nodes


def strict_check(rows, node_limit=2_000_000, max_depth=20):
    cells, n = parse_rows(rows)
    out = {'ok': False, 'reason': '', 'n': n}

    if n * n > 128:
        out['reason'] = 'unsupported_n'
        return out

    if not connected(cells):
        out['reason'] = 'P_disconnected'
        return out

    mnx, mxx, mny, mxy = bbox(cells)
    if (mnx, mxx, mny, mxy) != (0, n - 1, 0, n - 1):
        out['reason'] = 'bbox_not_full'
        return out

    r0 = cells
    r1 = rot90(cells, n)
    r3 = rot270(cells, n)

    if r0 == r1 or r0 == r3 or r1 == r3:
        out['reason'] = 'not_distinct_colors'
        return out

    q0 = r0 & r1 & r3
    if not q0:
        out['reason'] = 'q0_empty'
        return out
    qminx, qmaxx, qminy, qmaxy = bbox(q0)
    if (qmaxx - qminx + 1, qmaxy - qminy + 1) != (8, 8):
        out['reason'] = 'q0_not_8x8'
        return out

    qn = normalize(q0)
    if not connected(qn):
        out['reason'] = 'q0_disconnected'
        return out

    # stronger than pairwise: center + any subset of NESW neighbors
    dirs = [(8, 0), (-8, 0), (0, 8), (0, -8)]
    for mask in range(1 << 4):
        u = set(qn)
        for i, (dx, dy) in enumerate(dirs):
            if (mask >> i) & 1:
                u |= shift(qn, dx, dy)
        if not connected(u):
            out['reason'] = f'q0_pattern_disconnected_{mask}'
            return out

    a = (-qminx, -qminy)
    c0 = normalize(r0)
    c1 = normalize(r1)
    c2 = normalize(r3)

    p1 = shift(c0, a[0], a[1])
    p2 = shift(c1, a[0], a[1])
    p3 = shift(c2, a[0], a[1])
    regs = [p1, p2, p3]

    for dx, dy in dirs:
        for i in range(3):
            for j in range(3):
                ov = any((x + dx, y + dy) in regs[i] for x, y in regs[j])
                if ov != (i == j):
                    out['reason'] = 'property3_overlap'
                    return out

    all_or = unique_isometries_norm(cells, n)

    idxs = []
    for tgt in (c0, c1, c2):
        found = None
        ft = frozenset(tgt)
        for i, o in enumerate(all_or):
            if frozenset(o) == ft:
                found = i
                break
        if found is None:
            out['reason'] = 'orientation_missing'
            return out
        idxs.append(found)

    sq = single_covers(all_or, qn)
    exp_q = {(idxs[0], a[0], a[1]), (idxs[1], a[0], a[1]), (idxs[2], a[0], a[1])}
    if sq != exp_q:
        out['reason'] = 'single_q0'
        out['single_q0_count'] = len(sq)
        return out

    for k, reg in enumerate(regs):
        sp = single_covers(all_or, reg)
        exp = {(idxs[k], a[0], a[1])}
        if sp != exp:
            out['reason'] = f'single_p{k+1}'
            out[f'single_p{k+1}_count'] = len(sp)
            return out

    multi_nodes = {}
    for nm, reg in [('Q0', qn), ('P1', p1), ('P2', p2), ('P3', p3)]:
        st, nodes = has_multi(all_or, reg, node_limit=node_limit, max_depth=max_depth)
        multi_nodes[nm] = nodes
        if st != 'no':
            out['reason'] = f'multi_{nm}_{st}'
            out['multi_nodes'] = multi_nodes
            return out

    out['ok'] = True
    out['reason'] = 'ok'
    out['multi_nodes'] = multi_nodes
    return out


def hole_count(rows):
    cells, n = parse_rows(rows)
    mnx, mxx, mny, mxy = bbox(cells)
    mnx -= 1
    mxx += 1
    mny -= 1
    mxy += 1
    ext = set(cells)
    q = deque([(mnx, mny)])
    vis = {(mnx, mny)}
    while q:
        x, y = q.popleft()
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            nx, ny = x + dx, y + dy
            if nx < mnx or nx > mxx or ny < mny or ny > mxy:
                continue
            p = (nx, ny)
            if p in ext or p in vis:
                continue
            vis.add(p)
            q.append(p)
    holes = 0
    for y in range(mny, mxy + 1):
        for x in range(mnx, mxx + 1):
            p = (x, y)
            if p not in ext and p not in vis:
                holes += 1
    return holes


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--candidates', default=CAND_PATH, help='path to Rust JSON output')
    ap.add_argument('--node-limit', type=int, default=2_000_000)
    ap.add_argument('--max-depth', type=int, default=20)
    args = ap.parse_args()

    arr = json.load(open(args.candidates))
    oks = []
    fails = []
    for i, cand in enumerate(arr):
        r = strict_check(cand['rows'], node_limit=args.node_limit, max_depth=args.max_depth)
        if r['ok']:
            h = hole_count(cand['rows'])
            oks.append((h, cand['flips'], cand['seed_name'], i, cand, r))
        else:
            fails.append((i, cand, r))

    oks.sort(key=lambda x: (x[0], x[1], x[4]['size'], x[2], x[4]['rows']))
    print('CAND_PATH', args.candidates)
    print('TOTAL', len(arr))
    print('OK', len(oks))
    print('FAIL', len(fails))
    if fails:
        from collections import Counter
        ctr = Counter(f[2]['reason'] for f in fails)
        print('FAIL_REASONS', dict(ctr))
        print('FIRST_FAIL', fails[0][0], fails[0][2])

    print('BEST5')
    for h, flips, seed, i, cand, r in oks[:5]:
        print(f'idx={i} holes={h} flips={flips} seed={seed} size={cand["size"]} nodes={r["multi_nodes"]}')
        for row in cand['rows']:
            print(row)
        print('---')


if __name__ == '__main__':
    main()

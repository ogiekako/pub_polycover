#!/usr/bin/env python3
import random
import time
from collections import deque

SEED_ROWS_10 = [
    "0000001000",
    "0110001100",
    "0111111110",
    "0011111100",
    "0011111100",
    "1111111111",
    "0011011100",
    "0111111010",
    "0100001110",
    "0000001000",
]


def rows_to_set(rows):
    n = len(rows)
    s = set()
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            if ch == '1':
                s.add((x, y))
    return s, n


def set_to_rows(cells, n):
    rows = []
    for y in range(n):
        row = []
        for x in range(n):
            row.append('1' if (x, y) in cells else '0')
        rows.append(''.join(row))
    return rows


def rot90(cells, n):
    return {(n - 1 - y, x) for (x, y) in cells}


def rot180(cells, n):
    return {(n - 1 - x, n - 1 - y) for (x, y) in cells}


def rot270(cells, n):
    return {(y, n - 1 - x) for (x, y) in cells}


def reflx(cells, n):
    return {(n - 1 - x, y) for (x, y) in cells}


def d4_variants(cells, n):
    r0 = cells
    r1 = rot90(r0, n)
    r2 = rot90(r1, n)
    r3 = rot90(r2, n)
    f0 = reflx(r0, n)
    f1 = rot90(f0, n)
    f2 = rot90(f1, n)
    f3 = rot90(f2, n)
    variants = []
    seen = set()
    for v in [r0, r1, r2, r3, f0, f1, f2, f3]:
        key = frozenset(v)
        if key not in seen:
            seen.add(key)
            variants.append(v)
    return variants


def shift(cells, dx, dy):
    return {(x + dx, y + dy) for (x, y) in cells}


def bbox(cells):
    xs = [x for x, _ in cells]
    ys = [y for _, y in cells]
    return min(xs), min(ys), max(xs), max(ys)


def connected(cells):
    if not cells:
        return False
    q = deque([next(iter(cells))])
    vis = {q[0]}
    while q:
        x, y = q.popleft()
        for dx, dy in [(1,0),(-1,0),(0,1),(0,-1)]:
            np = (x+dx, y+dy)
            if np in cells and np not in vis:
                vis.add(np)
                q.append(np)
    return len(vis) == len(cells)


def intersects(a, b):
    if len(a) > len(b):
        a, b = b, a
    for c in a:
        if c in b:
            return True
    return False


def cond1_q0(P, n):
    Q0 = P & rot90(P, n) & rot270(P, n)
    if not Q0:
        return None
    mnx, mny, mxx, mxy = bbox(Q0)
    if (mxx - mnx + 1, mxy - mny + 1) != (8, 8):
        return None
    if not connected(Q0):
        return None
    return Q0


def cond2_adj_connect(Q0):
    for dx, dy in [(8,0),(-8,0),(0,8),(0,-8)]:
        U = Q0 | shift(Q0, dx, dy)
        if not connected(U):
            return False
    return True


def cond3_overlap(P, n):
    P1 = P
    P2 = rot90(P, n)
    P3 = rot270(P, n)
    Ps = [P1, P2, P3]
    for i in range(3):
        for j in range(3):
            for dx, dy in [(8,0),(-8,0),(0,8),(0,-8)]:
                ov = intersects(Ps[i], shift(Ps[j], dx, dy))
                if ov != (i == j):
                    return False
    return True


def all_placements_intersecting_target(P, n, target):
    varsP = d4_variants(P, n)
    txs = [x for x, _ in target]
    tys = [y for _, y in target]
    tminx, tmaxx = min(txs), max(txs)
    tminy, tmaxy = min(tys), max(tys)
    placements = []
    seen = set()
    for v in varsP:
        vxs = [x for x, _ in v]
        vys = [y for _, y in v]
        vminx, vmaxx = min(vxs), max(vxs)
        vminy, vmaxy = min(vys), max(vys)
        for dx in range(tminx - vmaxx, tmaxx - vminx + 1):
            for dy in range(tminy - vmaxy, tmaxy - vminy + 1):
                s = shift(v, dx, dy)
                if intersects(s, target):
                    key = frozenset(s)
                    if key not in seen:
                        seen.add(key)
                        placements.append(s)
    return placements


def single_cover_count(P, n, target):
    cnt = 0
    for pl in all_placements_intersecting_target(P, n, target):
        if target <= pl:
            cnt += 1
    return cnt


def has_multi_cover(P, n, target, max_nodes=2_000_000):
    placements = []
    for pl in all_placements_intersecting_target(P, n, target):
        if pl == target:
            continue
        cover = pl & target
        if cover:
            placements.append((pl, cover))
    target_list = sorted(target)
    idx = {c: i for i, c in enumerate(target_list)}
    full_mask = (1 << len(target_list)) - 1

    p2 = []
    for pl, cov in placements:
        m = 0
        for c in cov:
            m |= 1 << idx[c]
        p2.append((pl, m))

    by_cell = [[] for _ in range(len(target_list))]
    for i, (_, m) in enumerate(p2):
        mm = m
        while mm:
            b = mm & -mm
            ci = (b.bit_length() - 1)
            by_cell[ci].append(i)
            mm ^= b

    nodes = 0
    def dfs(covered, used, depth):
        nonlocal nodes
        nodes += 1
        if nodes > max_nodes:
            return False, True
        if covered == full_mask:
            return (depth >= 2), False
        rem = full_mask ^ covered
        b = rem & -rem
        need = b.bit_length() - 1
        for pi in by_cell[need]:
            pl, m = p2[pi]
            if m & covered == 0:
                ok = True
                for uj in used:
                    if intersects(pl, p2[uj][0]):
                        ok = False
                        break
                if not ok:
                    continue
                found, timeout = dfs(covered | m, used + [pi], depth + 1)
                if found or timeout:
                    return found, timeout
        return False, False

    found, timeout = dfs(0, [], 0)
    return found, timeout, nodes


def centered_rotations(P, n):
    return [P, rot90(P, n), rot270(P, n)]


def cond4_single(P, n, Q0):
    c_q0 = single_cover_count(P, n, Q0)
    if c_q0 != 3:
        return False, {"Q0_single": c_q0}
    Ps = centered_rotations(P, n)
    pc = []
    for Pi in Ps:
        pc.append(single_cover_count(P, n, Pi))
    ok = all(x == 1 for x in pc)
    return ok, {"Q0_single": c_q0, "Pi_single": pc}


def cond5_multi(P, n, Q0, max_nodes=5_000_000):
    status = {}
    m, t, nodes = has_multi_cover(P, n, Q0, max_nodes=max_nodes)
    status["Q0"] = {"multi": m, "timeout": t, "nodes": nodes}
    if m or t:
        return False, status
    for k, Pi in enumerate(centered_rotations(P, n), 1):
        m, t, nodes = has_multi_cover(P, n, Pi, max_nodes=max_nodes)
        status[f"P{k}"] = {"multi": m, "timeout": t, "nodes": nodes}
        if m or t:
            return False, status
    return True, status


def mutate(P, n, rate=2):
    P = set(P)
    for _ in range(rate):
        x = random.randrange(n)
        y = random.randrange(n)
        if (x, y) in P:
            P.remove((x, y))
        else:
            P.add((x, y))
    return P


def score_candidate(P, n):
    s = 0
    Q0 = P & rot90(P, n) & rot270(P, n)
    if Q0:
        mnx, mny, mxx, mxy = bbox(Q0)
        bw, bh = mxx - mnx + 1, mxy - mny + 1
        s -= abs(bw - 8) + abs(bh - 8)
        if connected(Q0):
            s += 10
    if cond3_overlap(P, n):
        s += 20
    return s


def run_search(n, seed_rows, iter_budget, max_nodes, rng_seed=0):
    random.seed(rng_seed + n)
    seedP, _ = rows_to_set(seed_rows)
    if n != len(seed_rows):
        P = set()
        off = (n - len(seed_rows)) // 2
        for x, y in seedP:
            P.add((x + off, y + off))
    else:
        P = set(seedP)

    best = None
    best_diag = None

    def eval_full(P):
        Q0 = cond1_q0(P, n)
        if Q0 is None:
            return None, {"ok1": False}
        if not cond2_adj_connect(Q0):
            return None, {"ok1": True, "ok2": False}
        if not cond3_overlap(P, n):
            return None, {"ok1": True, "ok2": True, "ok3": False}
        ok4, d4 = cond4_single(P, n, Q0)
        if not ok4:
            dd = {"ok1": True, "ok2": True, "ok3": True, "ok4": False}
            dd.update(d4)
            return None, dd
        ok5, d5 = cond5_multi(P, n, Q0, max_nodes=max_nodes)
        dd = {"ok1": True, "ok2": True, "ok3": True, "ok4": True, "ok5": ok5}
        dd.update(d4)
        dd["multi"] = d5
        if ok5:
            return P, dd
        return None, dd

    cand, diag = eval_full(P)
    best = set(P)
    best_diag = diag
    if cand is not None:
        return cand, diag, 0

    cur = set(P)
    cur_score = score_candidate(cur, n)

    for it in range(1, iter_budget + 1):
        mrate = 1 if random.random() < 0.7 else 2
        nxt = mutate(cur, n, rate=mrate)
        sc = score_candidate(nxt, n)
        if sc >= cur_score or random.random() < 0.02:
            cur, cur_score = nxt, sc
        if best is None or sc > score_candidate(best, n):
            best = set(nxt)
        if it % 200 == 0:
            cand, diag = eval_full(cur)
            if diag:
                best_diag = diag
            if cand is not None:
                return cand, diag, it

    cand, diag = eval_full(best)
    if diag:
        best_diag = diag
    return cand, (diag if diag else best_diag), iter_budget


def main():
    t0 = time.time()
    result = {"found": False, "n": None, "rows": None, "diag": None, "iters": None}

    plans = [
        (10, 60000, 8_000_000),
        (11, 80000, 10_000_000),
        (12, 100000, 12_000_000),
        (13, 120000, 15_000_000),
    ]

    for n, iters, nodes in plans:
        cand, diag, used = run_search(n, SEED_ROWS_10, iter_budget=iters, max_nodes=nodes, rng_seed=42)
        print(f"[n={n}] iters={used} diag={diag}")
        if cand is not None:
            result.update({"found": True, "n": n, "rows": set_to_rows(cand, n), "diag": diag, "iters": used})
            break
        if result["diag"] is None:
            result.update({"n": n, "diag": diag, "iters": used, "rows": set_to_rows(set(rows_to_set(SEED_ROWS_10)[0]), len(SEED_ROWS_10)) if n==10 else None})

    print("===RESULT===")
    print(result)
    print(f"elapsed_sec={time.time()-t0:.2f}")


if __name__ == '__main__':
    main()

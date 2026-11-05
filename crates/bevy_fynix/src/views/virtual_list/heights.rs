//! The heights of a virtual list's rows, summed in blocks.

/// How many rows a block holds when a run of rows is cut into blocks.
const ROWS_PER_BLOCK: usize = 512;

/// The fewest rows a splice leaves its blocks with before it takes a
/// neighbouring block in, so blocks never dwindle.
const MIN_BLOCK: usize = ROWS_PER_BLOCK / 2;

/// The height of every row, measured or estimated, in blocks of
/// about [`ROWS_PER_BLOCK`] rows under a Fenwick tree of their sums.
/// A row's offset is a walk down the tree and along one block, a
/// measured height updates one path up the tree, and a splice
/// rebuilds only the blocks it covers, so a list of 100,000 rows
/// takes a splice or a scroll in its stride.
#[derive(Default)]
pub(super) struct Heights {
    blocks: Vec<Block>,
    tree: Vec<Sum>,
    len: usize,
}

/// A run of rows: each one's height and whether it was measured, and
/// their sum.
struct Block {
    rows: Vec<(f32, bool)>,
    px: f64,
}

/// A node of the tree: the rows and pixels of the blocks it sums.
#[derive(Clone, Copy, Default)]
struct Sum {
    rows: usize,
    px: f64,
}

impl Block {
    fn new(rows: Vec<(f32, bool)>) -> Self {
        let px =
            rows.iter().map(|&(height, _)| f64::from(height)).sum();
        Self { rows, px }
    }

    fn offset(&self, row: usize, above: f64) -> f64 {
        self.rows[..row]
            .iter()
            .fold(above, |at, &(height, _)| at + f64::from(height))
    }
}

impl Heights {
    pub(super) fn len(&self) -> usize {
        self.len
    }

    pub(super) fn total(&self) -> f64 {
        self.offset(self.len)
    }

    /// Adds rows of `estimate` at the end or removes rows from it,
    /// to hold `count`.
    pub(super) fn resize(&mut self, count: usize, estimate: f32) {
        let len = self.len;
        self.splice(
            count.min(len),
            len.saturating_sub(count),
            count.saturating_sub(len),
            estimate,
        );
    }

    /// Replaces the rows `at..at + removed` with `inserted` rows of
    /// `estimate`, rebuilding only the blocks they lie in.
    pub(super) fn splice(
        &mut self,
        at: usize,
        removed: usize,
        inserted: usize,
        estimate: f32,
    ) {
        let end = at + removed;
        let (mut first, mut start) = match self.locate(at) {
            (first, _, _)
                if first == self.blocks.len() && first > 0 =>
            {
                (first - 1, at - self.blocks[first - 1].rows.len())
            }
            (first, row, _) => (first, at - row),
        };
        let mut stop = match self.locate(end) {
            (last, 0, _) if last > first => last,
            (last, _, _) => (last + 1).min(self.blocks.len()),
        };
        let run = self.blocks[first..stop]
            .iter()
            .map(|block| block.rows.len())
            .sum::<usize>();
        if run - removed + inserted < MIN_BLOCK {
            if stop < self.blocks.len() {
                stop += 1;
            } else if first > 0 {
                first -= 1;
                start -= self.blocks[first].rows.len();
            }
        }
        let mut rows = self.blocks[first..stop]
            .iter()
            .flat_map(|block| block.rows.iter().copied())
            .collect::<Vec<_>>();
        rows.splice(
            at - start..end - start,
            core::iter::repeat_n((estimate, false), inserted),
        );
        let pieces = into_blocks(rows);
        let same = pieces.len() == stop - first;
        if same {
            for (block, piece) in (first..).zip(&pieces) {
                let was = &self.blocks[block];
                let rows = piece.rows.len() as isize
                    - was.rows.len() as isize;
                let px = piece.px - was.px;
                self.add(block, rows, px);
            }
        }
        self.blocks.splice(first..stop, pieces);
        if !same {
            self.rebuild();
        }
        self.len = self.len - removed + inserted;
    }

    /// Gives the row `index` its measured height.
    pub(super) fn set(&mut self, index: usize, height: f32) {
        let (block, row, _) = self.locate(index);
        let (was, known) = &mut self.blocks[block].rows[row];
        *known = true;
        if *was == height {
            return;
        }
        let by = f64::from(height) - f64::from(*was);
        *was = height;
        self.blocks[block].px += by;
        self.add(block, 0, by);
    }

    /// Leaves every row its height as an estimate only.
    pub(super) fn forget(&mut self) {
        for block in &mut self.blocks {
            for (_, known) in &mut block.rows {
                *known = false;
            }
        }
    }

    /// Gives every row not measured the height `estimate`.
    pub(super) fn reestimate(&mut self, estimate: f32) {
        for block in &mut self.blocks {
            for (height, known) in &mut block.rows {
                if !*known {
                    *height = estimate;
                }
            }
            *block = Block::new(core::mem::take(&mut block.rows));
        }
        self.rebuild();
    }

    fn add(&mut self, block: usize, rows: isize, px: f64) {
        let mut k = block + 1;
        while k < self.tree.len() {
            self.tree[k].rows =
                self.tree[k].rows.wrapping_add_signed(rows);
            self.tree[k].px += px;
            k += k.isolate_lowest_one();
        }
    }

    fn rebuild(&mut self) {
        self.tree.clear();
        self.tree.push(Sum::default());
        self.tree.extend(self.blocks.iter().map(|block| Sum {
            rows: block.rows.len(),
            px: block.px,
        }));
        for k in 1..self.tree.len() {
            let up = k + k.isolate_lowest_one();
            if let Some(&below) = self.tree.get(k)
                && let Some(sum) = self.tree.get_mut(up)
            {
                sum.rows += below.rows;
                sum.px += below.px;
            }
        }
    }

    /// The block the row `index` is in, the row's place in it, and
    /// the pixels above the block.
    fn locate(&self, index: usize) -> (usize, usize, f64) {
        let (mut block, mut rows, mut px) = (0, 0, 0.0);
        let mut step = self.top_step();
        while step > 0 {
            let next = block + step;
            if next < self.tree.len()
                && rows + self.tree[next].rows <= index
            {
                block = next;
                rows += self.tree[next].rows;
                px += self.tree[next].px;
            }
            step >>= 1;
        }
        (block, index - rows, px)
    }

    fn above(&self, block: usize) -> f64 {
        let (mut at, mut px) = (0, 0.0);
        let mut step = self.top_step();
        while step > 0 {
            if at + step <= block {
                at += step;
                px += self.tree[at].px;
            }
            step >>= 1;
        }
        px
    }

    fn top_step(&self) -> usize {
        self.blocks.len().checked_ilog2().map_or(0, |log| 1 << log)
    }

    /// The row's height and whether it was measured, `None` past the
    /// end.
    pub(super) fn row(&self, index: usize) -> Option<(f32, bool)> {
        let (block, row, _) = self.locate(index);
        self.blocks.get(block)?.rows.get(row).copied()
    }

    /// The pixels above the row `index`, the total at the end.
    pub(super) fn offset(&self, index: usize) -> f64 {
        let (block, row, above) = self.locate(index);
        self.blocks
            .get(block)
            .map_or(above, |block| block.offset(row, above))
    }

    /// The offsets of the row `from` and every row after it, then
    /// the total.
    pub(super) fn offsets(
        &self,
        from: usize,
    ) -> impl Iterator<Item = f64> + '_ {
        let (mut block, mut row, _) = self.locate(from);
        let mut at = self.offset(from);
        core::iter::from_fn(move || {
            let offset = at;
            if let Some(rows) = self.blocks.get(block) {
                if row + 1 < rows.rows.len() {
                    at += f64::from(rows.rows[row].0);
                    row += 1;
                } else {
                    block += 1;
                    row = 0;
                    at = self.above(block);
                }
            }
            Some(offset)
        })
    }

    /// The last row that starts at or above `y`, the first when `y`
    /// is above every row.
    pub(super) fn index_at(&self, y: f64) -> usize {
        let (mut block, mut rows, mut px) = (0, 0, 0.0);
        let mut step = self.top_step();
        while step > 0 {
            let next = block + step;
            if next < self.tree.len() && px + self.tree[next].px <= y
            {
                block = next;
                rows += self.tree[next].rows;
                px += self.tree[next].px;
            }
            step >>= 1;
        }
        let Some(block) = self.blocks.get(block) else {
            return self.len.saturating_sub(1);
        };
        let last = block.rows.len() - 1;
        let mut row = 0;
        for &(height, _) in &block.rows[..last] {
            px += f64::from(height);
            if px > y {
                break;
            }
            row += 1;
        }
        rows + row
    }
}

/// Cuts `rows` into blocks of about [`ROWS_PER_BLOCK`], or one block
/// while they are few.
fn into_blocks(rows: Vec<(f32, bool)>) -> Vec<Block> {
    if rows.is_empty() {
        return Vec::new();
    }
    if rows.len() <= 2 * ROWS_PER_BLOCK {
        return vec![Block::new(rows)];
    }
    let count = rows.len().div_ceil(ROWS_PER_BLOCK);
    (0..count)
        .map(|piece| {
            let from = piece * rows.len() / count;
            let to = (piece + 1) * rows.len() / count;
            Block::new(rows[from..to].to_vec())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ESTIMATE: f32 = 20.0;

    fn splitmix64(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(state: &mut u64, bound: usize) -> usize {
        (splitmix64(state) % bound as u64) as usize
    }

    /// The same rows kept plainly: each one's height and whether it
    /// was measured, and the pixels above each row and the total.
    struct Plain {
        rows: Vec<(f32, bool)>,
        offsets: Vec<f64>,
    }

    impl Plain {
        fn new(rows: Vec<(f32, bool)>) -> Self {
            let offsets = core::iter::once(0.0)
                .chain(rows.iter().scan(0.0, |at, &(height, _)| {
                    *at += f64::from(height);
                    Some(*at)
                }))
                .collect();
            Self { rows, offsets }
        }

        fn index_at(&self, y: f64) -> usize {
            (0..self.rows.len())
                .rev()
                .find(|&index| self.offsets[index] <= y)
                .unwrap_or(0)
        }
    }

    #[test]
    fn random_splices_and_measures_match_a_plain_list() {
        let mut random = 1;
        let mut rows = vec![(ESTIMATE, false); 2_500];
        let mut heights = Heights::default();
        heights.resize(rows.len(), ESTIMATE);
        for step in 0..200 {
            let len = rows.len();
            let estimate = ESTIMATE + below(&mut random, 4) as f32;
            if below(&mut random, 6) == 0 {
                let count = below(&mut random, 6_001);
                heights.resize(count, estimate);
                rows.resize(count, (estimate, false));
            } else {
                let most = if below(&mut random, 8) == 0 {
                    1_500
                } else {
                    8
                };
                let at = below(&mut random, len + 1);
                let removed =
                    below(&mut random, most.min(len - at) + 1);
                let mut inserted = below(&mut random, most + 1);
                if len - removed + inserted > 6_000 {
                    inserted = 0;
                }
                heights.splice(at, removed, inserted, estimate);
                rows.splice(
                    at..at + removed,
                    core::iter::repeat_n((estimate, false), inserted),
                );
            }
            for _ in 0..below(&mut random, 20).min(rows.len()) {
                let index = below(&mut random, rows.len());
                let height =
                    8.0 + below(&mut random, 256) as f32 / 8.0;
                heights.set(index, height);
                rows[index] = (height, true);
            }
            if below(&mut random, 10) == 0 {
                heights.reestimate(estimate);
                for (height, known) in &mut rows {
                    if !*known {
                        *height = estimate;
                    }
                }
            }
            if below(&mut random, 20) == 0 {
                heights.forget();
                for (_, known) in &mut rows {
                    *known = false;
                }
            }

            let when = format!("step {step}");
            let plain = Plain::new(rows.clone());
            let len = rows.len();
            assert_eq!(heights.len(), len, "{when}");
            assert_eq!(heights.total(), plain.offsets[len], "{when}");
            assert_eq!(heights.row(len), None, "{when}");
            for _ in 0..50 {
                let index = below(&mut random, len + 1);
                assert_eq!(
                    heights.offset(index),
                    plain.offsets[index],
                    "{when}: the offset of row {index}"
                );
                assert_eq!(
                    heights.row(index),
                    rows.get(index).copied(),
                    "{when}: row {index}"
                );
                let y = below(
                    &mut random,
                    plain.offsets[len] as usize * 8 + 800,
                ) as f64
                    / 8.0;
                assert_eq!(
                    heights.index_at(y),
                    plain.index_at(y),
                    "{when}: the row at {y}"
                );
            }
            let from = below(&mut random, len + 1);
            let walked = heights
                .offsets(from)
                .take((len + 1 - from).min(600))
                .collect::<Vec<_>>();
            assert_eq!(
                walked,
                plain.offsets[from..(from + 600).min(len + 1)],
                "{when}: the walk from {from}"
            );
        }
    }
}

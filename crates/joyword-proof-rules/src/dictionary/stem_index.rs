//! A compact index over an existing byte-sorted pool of unique dictionary stems.
//! Buckets store row numbers in lexical order. An uneven distribution costs at
//! most a binary search within a bucket and never requires a growing probe walk.

#[derive(Clone, Debug, Default)]
pub struct StemIndex {
    starts: Vec<u32>,
    rows: Vec<u32>,
    tags: Vec<u8>,
}

fn word<'a>(pool: &'a str, offsets: &[u32], i: usize) -> Option<&'a str> {
    let bounds = offsets.get(i..=i.checked_add(1)?)?;
    pool.get(bounds[0] as usize..bounds[1] as usize)
}

// Mix complete eight-byte lanes. The low byte filters candidate rows; high bits
// select the bucket. Equal fingerprints are always checked against the full word.
fn fingerprint(bytes: &[u8]) -> u64 {
    const MIX: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut h = bytes.len() as u64;
    let (lanes, remainder) = bytes.as_chunks::<8>();
    for &lane in lanes {
        h = (h ^ u64::from_le_bytes(lane))
            .wrapping_mul(MIX)
            .rotate_left(29);
    }
    let mut tail = [0; 8];
    tail[..remainder.len()].copy_from_slice(remainder);
    h = (h ^ u64::from_le_bytes(tail)).wrapping_mul(MIX);
    h ^ (h >> 33)
}

impl StemIndex {
    /// Offsets address the start of each stem and end with the byte length of the pool.
    pub fn build(pool: &str, offsets: &[u32]) -> Result<Self, &'static str> {
        if offsets.first() != Some(&0)
            || offsets.last().copied().map(|i| i as usize) != Some(pool.len())
        {
            return Err("invalid stem offsets");
        }
        let count = offsets.len() - 1;
        let count32 = u32::try_from(count).map_err(|_| "too many stems")?;
        let buckets = count.div_ceil(4).max(2).next_power_of_two();
        let bits = buckets.ilog2();
        let mut starts = vec![0u32; buckets + 1];
        let mut previous = None;
        for i in 0..count {
            let stem = word(pool, offsets, i).ok_or("invalid stem boundary")?;
            if previous.is_some_and(|last| last >= stem) {
                return Err("stem pool is not strictly sorted");
            }
            previous = Some(stem);
            starts[(fingerprint(stem.as_bytes()) >> (64 - bits)) as usize] += 1;
        }
        let mut seen = 0;
        for n in &mut starts {
            let size = *n;
            *n = seen;
            seen += size;
        }
        let mut rows = vec![0; count];
        let mut tags = vec![0; count];
        // Use starts as insertion cursors; one shift restores the original boundaries.
        for i in 0..count32 {
            let stem = word(pool, offsets, i as usize).ok_or("invalid stem boundary")?;
            let hash = fingerprint(stem.as_bytes());
            let cursor = &mut starts[(hash >> (64 - bits)) as usize];
            tags[*cursor as usize] = hash as u8;
            rows[*cursor as usize] = i;
            *cursor += 1;
        }
        starts.copy_within(..buckets, 1);
        starts[0] = 0;
        Ok(Self { starts, rows, tags })
    }

    pub fn find(&self, pool: &str, offsets: &[u32], needle: &str) -> Option<u32> {
        let buckets = self.starts.len().checked_sub(1)?;
        let hash = fingerprint(needle.as_bytes());
        let b = (hash >> (64 - buckets.ilog2())) as usize;
        let start = self.starts[b] as usize;
        let end = self.starts[b + 1] as usize;
        let rows = self.rows.get(start..end)?;
        if rows.len() <= 12 {
            for (&tag, &row) in self.tags[start..end].iter().zip(rows) {
                if tag == hash as u8 && word(pool, offsets, row as usize) == Some(needle) {
                    return Some(row);
                }
            }
            return None;
        }
        let pos = rows
            .binary_search_by(|&i| word(pool, offsets, i as usize).cmp(&Some(needle)))
            .ok()?;
        Some(rows[pos])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_unicode_rows_and_large_shared_prefixes() {
        let mut roots: Vec<String> = (0..2000).map(|i| format!("group{i:04}same")).collect();
        roots.extend(["", "a", "ab", "abc", "ç", "辞書", "İ", "🍎"].map(str::to_owned));
        roots.sort();
        let pool = roots.concat();
        let mut offsets = vec![0];
        for word in &roots {
            offsets.push(offsets.last().copied().unwrap_or(0) + word.len() as u32);
        }
        let index = StemIndex::build(&pool, &offsets).unwrap();
        for (i, word) in roots.iter().enumerate() {
            assert_eq!(index.find(&pool, &offsets, word), Some(i as u32));
        }
        for missing in ["group2999same", "group1999sam", "辞", "è", "abcd"] {
            assert_eq!(index.find(&pool, &offsets, missing), None);
        }
        assert!(index.rows.len() + index.starts.len() < roots.len() * 2);
        assert!(StemIndex::build("é", &[0, 1, 2]).is_err());
        assert!(StemIndex::build("ba", &[0, 1, 2]).is_err());
        assert!(StemIndex::build("aa", &[0, 1, 2]).is_err());
        assert!(
            StemIndex::build("", &[0])
                .unwrap()
                .find("", &[0], "x")
                .is_none()
        );
    }
}

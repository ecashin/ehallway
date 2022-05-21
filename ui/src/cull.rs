// It would be nice to use tallystick, but I don't want to use nightly.

pub struct Ranking {
    // Entries are ordered to correspond to an array of choices.
    // Values are scores, with higher scores preferred.
    // Only the score order is used to determine the ranking.
    pub scores: Vec<usize>,
}

fn argsort(a: &[usize]) -> Vec<usize> {
    let mut indexed: Vec<_> = a.iter().enumerate().collect();
    indexed.sort_by(|(_ai, av), (_bi, bv)| av.partial_cmp(bv).unwrap());
    indexed.iter().map(|(i, _v)| *i).collect::<Vec<usize>>()
}

pub fn borda_count(rankings: &[Ranking]) -> Vec<usize> {
    let rankings: Vec<_> = rankings.iter().map(|r| argsort(&r.scores)).collect();
    todo!()
}

#[cfg(test)]
mod tests {
    use super::{argsort, borda_count};

    #[test]
    fn test_argsort() {
        let a: Vec<_> = (0..10).collect();
        let b = a.clone();
        let i = argsort(&b);
        let bb: Vec<_> = i.iter().map(|j| b[*j]).collect();
        assert_eq!(a.len(), bb.len());
        for (i, j) in a.iter().zip(bb.iter()) {
            assert_eq!(i, j);
        }
    }
}

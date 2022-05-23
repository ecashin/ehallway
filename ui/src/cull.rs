// It would be nice to use tallystick, but I don't want to use nightly.
use anyhow::{anyhow, Result};

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

pub fn borda_count(rankings: &[Ranking]) -> Result<Vec<usize>> {
    if rankings.is_empty() {
        return Ok(vec![]);
    }
    let off0 = &rankings[..rankings.len() - 1];
    let off1 = &rankings[1..];
    let sum_diffs = off0
        .iter()
        .zip(off1.iter())
        .map(|(a, b)| (a.scores.len() as isize - b.scores.len() as isize).pow(2))
        .sum::<isize>();
    if sum_diffs != 0 {
        return Err(anyhow!("lengths of rankings differ"));
    }
    let rankings: Vec<_> = rankings.iter().map(|r| argsort(&r.scores)).collect();
    let mut scores: Vec<_> = vec![];
    for i in 0..rankings[0].len() {
        scores.push((0..rankings.len()).map(|j| rankings[j][i]).sum());
    }
    Ok(scores)
}

#[cfg(test)]
mod tests {
    use super::{argsort, borda_count, Ranking};

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

    #[test]
    fn test_borda_count_agree() {
        let rankings = [
            Ranking {
                scores: vec![0, 1, 2],
            },
            Ranking {
                scores: vec![3, 4, 5],
            },
            Ranking {
                scores: vec![6, 7, 8],
            },
        ];
        let count = borda_count(&rankings).unwrap();
        assert_eq!(count, [0, 3, 6]);
    }

    #[test]
    fn test_borda_count_disagree() {
        let rankings = [
            Ranking {
                scores: vec![0, 1, 2],
            },
            Ranking {
                scores: vec![3, 4, 5],
            },
            Ranking {
                scores: vec![8, 7, 6],
            },
        ];
        let count = borda_count(&rankings).unwrap();
        assert_eq!(count, [2, 3, 4]);
    }
}

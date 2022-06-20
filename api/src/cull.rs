// It would be nice to use tallystick, but I don't want to use nightly.
use anyhow::{anyhow, Result};

use ehall::argsort;

#[derive(Clone, Debug)]
pub struct Ranking {
    // Entries are ordered to correspond to an array of choices.
    // Values are scores, with higher scores preferred.
    // Only the score order is used to determine the ranking.
    pub scores: Vec<usize>,
}

pub fn borda_count(rankings: &[Ranking]) -> Result<Vec<usize>> {
    if rankings.is_empty() {
        return Ok(vec![]);
    }
    let len = rankings[0].scores.len();
    for r in rankings.iter().skip(1) {
        if r.scores.len() != len {
            return Err(anyhow!("lengths of rankings differ"));
        }
    }

    // The most esteemed choice has the highest score and the lowest implicit rank.
    // Using argsort provides the conversion
    // from arbitrary scores to Borda-count points.
    let rankings: Vec<_> = rankings.iter().map(|r| argsort(&r.scores)).collect();
    let mut scores: Vec<_> = vec![];
    for j in 0..rankings[0].len() {
        scores.push((0..rankings.len()).map(|i| rankings[i][j]).sum());
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
    fn test_borda_one_ranking() {
        let rankings = [
            Ranking {
                scores: vec![9, 5, 11, 0, 4, 6, 8, 1, 7, 2, 3, 10],
            },
            Ranking {
                scores: vec![0, 1, 2],
            },
            Ranking {
                scores: vec![3, 5, 4],
            },
            Ranking {
                scores: vec![8, 7, 6],
            },
        ];
        for r in rankings.into_iter() {
            let rr = &[r.clone()];
            let count = borda_count(rr).unwrap();
            let i_expected = argsort(&r.scores);
            let i_observed = argsort(&count);
            assert_eq!(i_expected, i_observed);
        }
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

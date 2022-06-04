use anyhow::{anyhow, Result};
use rand::{seq::SliceRandom, thread_rng};

pub fn cohorts(n_participants: usize, cohort_size: usize) -> Result<Vec<Vec<usize>>> {
    if cohort_size > n_participants {
        return Err(anyhow!(
            "not enough participants ({}) for a cohort",
            n_participants
        ));
    }
    let rng = &mut thread_rng();
    let mut order: Vec<usize> = (0..n_participants).collect();
    order.shuffle(rng);
    Ok(order
        .chunks(cohort_size)
        .map(|cohort| cohort.to_vec())
        .collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::cohorts;
    use anyhow::Result;

    #[test]
    fn test_cohorts() -> Result<()> {
        let mut c = cohorts(3, 1)?;
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].len(), 1);
        c = cohorts(3, 2)?;
        println!("{:?}", c);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].len(), 2);
        assert_eq!(c[1].len(), 1);
        Ok(())
    }
}

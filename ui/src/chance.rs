use anyhow::{Context, Result};

pub fn cohorts(n_participants: usize, n_cohorts: usize) -> Result<()> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::cohorts;
    use anyhow::Result;

    #[test]
    fn test_cohorts() -> Result<()> {
        cohorts(3, 1)?;
        Ok(())
    }
}

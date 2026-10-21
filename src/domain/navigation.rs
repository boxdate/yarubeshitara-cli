use crate::domain::date::LocalDate;

/// Given a sorted (or unsorted) slice of recorded dates and a `base_date`,
/// resolves the target recorded date for a relative step `offset` (e.g. -1 for `prev`, +1 for `next`).
///
/// - When `offset < 0` (e.g. `-1` / `prev`):
///   Selects the `|offset|`-th recorded date strictly before `base_date` (in reverse chronological order).
///   This skips unrecorded days/holidays so `bujo prev` on Monday jumps directly to Friday's log (TP-14 / BDD 04-1).
/// - When `offset > 0` (e.g. `+1` / `next`):
///   Selects the `offset`-th recorded date strictly after `base_date` (in chronological order).
/// - When `offset == 0`:
///   Returns `Some(base_date)`.
pub fn resolve_relative_recorded_date(
    recorded_dates: &[LocalDate],
    base_date: LocalDate,
    offset: isize,
) -> Option<LocalDate> {
    if offset == 0 {
        return Some(base_date);
    }

    let mut sorted: Vec<LocalDate> = recorded_dates.to_vec();
    sorted.sort();
    sorted.dedup();

    if offset < 0 {
        let steps = (-offset) as usize;
        let past: Vec<LocalDate> = sorted.into_iter().filter(|&d| d < base_date).collect();
        if steps == 0 || steps > past.len() {
            None
        } else {
            Some(past[past.len() - steps])
        }
    } else {
        let steps = offset as usize;
        let future: Vec<LocalDate> = sorted.into_iter().filter(|&d| d > base_date).collect();
        future.get(steps - 1).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_relative_recorded_date_skips_gaps() {
        // Friday 10/16, Tuesday 10/20, Wednesday 10/21
        let d_fri = LocalDate::new(2026, 10, 16);
        let d_tue = LocalDate::new(2026, 10, 20);
        let d_wed = LocalDate::new(2026, 10, 21);
        let dates = vec![d_wed, d_fri, d_tue];

        // From Tuesday 10/20, -1 (prev) jumps to Friday 10/16 (1 business day / recorded day prior)
        assert_eq!(resolve_relative_recorded_date(&dates, d_tue, -1), Some(d_fri));
        assert_eq!(resolve_relative_recorded_date(&dates, d_tue, -2), None);
        // +1 (next) from Tuesday 10/20 jumps to Wednesday 10/21
        assert_eq!(resolve_relative_recorded_date(&dates, d_tue, 1), Some(d_wed));
        // From Wednesday 10/21, -2 jumps to Friday 10/16
        assert_eq!(resolve_relative_recorded_date(&dates, d_wed, -2), Some(d_fri));
    }
}

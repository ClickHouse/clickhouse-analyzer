/// Interval unit keywords supported by ClickHouse.
/// See: https://clickhouse.com/docs/en/sql-reference/data-types/special-data-types/interval
#[derive(Debug, Copy, Clone)]
pub enum IntervalUnit {
    Nanosecond,
    Microsecond,
    Millisecond,
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

impl IntervalUnit {
    pub fn from_str(s: &str) -> Option<IntervalUnit> {
        match s.to_ascii_uppercase().as_str() {
            "NANOSECOND" | "NANOSECONDS" | "NS" => Some(IntervalUnit::Nanosecond),
            "MICROSECOND" | "MICROSECONDS" => Some(IntervalUnit::Microsecond),
            "MILLISECOND" | "MILLISECONDS" | "MS" => Some(IntervalUnit::Millisecond),
            "SECOND" | "SECONDS" | "SS" | "S" => Some(IntervalUnit::Second),
            "MINUTE" | "MINUTES" | "MI" | "N" => Some(IntervalUnit::Minute),
            "HOUR" | "HOURS" | "HH" | "H" => Some(IntervalUnit::Hour),
            "DAY" | "DAYS" | "DD" | "D" => Some(IntervalUnit::Day),
            "WEEK" | "WEEKS" | "WK" | "WW" => Some(IntervalUnit::Week),
            "MONTH" | "MONTHS" | "MM" | "M" => Some(IntervalUnit::Month),
            "QUARTER" | "QUARTERS" | "QQ" | "Q" => Some(IntervalUnit::Quarter),
            "YEAR" | "YEARS" | "YYYY" | "YY" => Some(IntervalUnit::Year),
            _ => None,
        }
    }
}

use std::ops::RangeInclusive;

use chrono::{NaiveDate, NaiveTime, Utc};

pub const FIRST_DATE: NaiveDate =
    NaiveDate::from_ymd_opt(1978, 6, 19).expect("Failed to parse const date");

pub fn latest() -> NaiveDate {
    // TODO(refactor)
    use chrono::Duration;

    let now = Utc::now();

    // Get naive time (UTC) for when comic is published to gocomics.com
    // Estimated time is:
    //      0000-0300 EST
    //      0400-0700 UTC
    //      1400-1700 AEST
    // And a margin of error is added just in case
    let time_of_publish = NaiveTime::from_hms_opt(7, 0, 0)
        .expect("Static time failed to parse. This error should never occur.");

    // Today if currently AFTER time of publish for todays comic
    // Yesterday if currently BEFORE time of publish for todays comic
    now.date_naive() - Duration::days(if now.time() > time_of_publish { 0 } else { 1 })
}

pub fn date_iter(range: RangeInclusive<NaiveDate>) -> impl Iterator<Item = NaiveDate> {
    // TODO(refactor)
    use chrono::Duration;

    let (start, end) = (*range.start(), *range.end());
    (0..=(end - start).num_days()).map(move |days| start + Duration::days(days))
}

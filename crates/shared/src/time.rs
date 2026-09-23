//! Ora României (Europe/Bucharest) fără baza de date de fusuri: EET (+2) iarna, EEST (+3) vara, cu
//! regula UE: ora de vară începe în ultima duminică din martie la 01:00 UTC și se termină în ultima
//! duminică din octombrie la 01:00 UTC. Suficient pentru afișarea momentelor de sincronizare; se
//! compilează și la wasm, fără dependențe în plus.

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, Utc};

const LUNI: [&str; 12] = [
    "ianuarie",
    "februarie",
    "martie",
    "aprilie",
    "mai",
    "iunie",
    "iulie",
    "august",
    "septembrie",
    "octombrie",
    "noiembrie",
    "decembrie",
];

/// „16 septembrie 2026”
pub fn fmt_date_ro(d: NaiveDate) -> String {
    format!("{} {} {}", d.day(), LUNI[d.month0() as usize], d.year())
}

fn last_sunday(year: i32, month: u32) -> NaiveDate {
    let first_of_next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .expect("dată validă");
    let last_day = first_of_next.pred_opt().expect("dată validă");
    last_day - Duration::days(i64::from(last_day.weekday().num_days_from_sunday()))
}

/// Decalajul orei României față de UTC, în ore, la momentul dat.
pub fn romania_offset_hours(t: DateTime<Utc>) -> i64 {
    let y = t.year();
    let start = last_sunday(y, 3).and_hms_opt(1, 0, 0).expect("oră validă").and_utc();
    let end = last_sunday(y, 10).and_hms_opt(1, 0, 0).expect("oră validă").and_utc();
    if t >= start && t < end { 3 } else { 2 }
}

/// Momentul în ora României, fără fus atașat, pentru afișare.
pub fn to_romania_local(t: DateTime<Utc>) -> NaiveDateTime {
    (t + Duration::hours(romania_offset_hours(t))).naive_utc()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn iarna_plus_doua_ore_vara_plus_trei() {
        assert_eq!(
            to_romania_local(utc("2026-01-15T10:00:00Z")).to_string(),
            "2026-01-15 12:00:00"
        );
        assert_eq!(
            to_romania_local(utc("2026-07-15T10:00:00Z")).to_string(),
            "2026-07-15 13:00:00"
        );
    }

    #[test]
    fn limitele_orei_de_vara_2026() {
        // ultima duminică din martie 2026 e 29, din octombrie e 25; schimbarea e la 01:00 UTC
        assert_eq!(romania_offset_hours(utc("2026-03-29T00:59:59Z")), 2);
        assert_eq!(romania_offset_hours(utc("2026-03-29T01:00:00Z")), 3);
        assert_eq!(romania_offset_hours(utc("2026-10-25T00:59:59Z")), 3);
        assert_eq!(romania_offset_hours(utc("2026-10-25T01:00:00Z")), 2);
    }
}

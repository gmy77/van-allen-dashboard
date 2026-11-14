//! Utilità di tempo UTC senza dipendenze esterne.
//!
//! Tutti i timestamp del progetto sono epoch Unix in secondi, UTC.

/// Secondi dall'epoch Unix, adesso.
pub fn now_unix() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(_) => 0,
    }
}

/// Giorni civili dal 1970-01-01 (algoritmo di Howard Hinnant).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = ((m + 9) % 12) as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// (anno, mese, giorno, ora, minuto, secondo) da epoch Unix.
pub fn civil_from_unix(epoch: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = epoch.div_euclid(86_400);
    let secs = epoch.rem_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (
        y,
        m,
        d,
        (secs / 3600) as u32,
        ((secs % 3600) / 60) as u32,
        (secs % 60) as u32,
    )
}

/// Interpreta `2026-11-14T11:04:00Z`, `2026-11-14 11:04:00` e varianti
/// con o senza secondi/fuso. Restituisce None se non riconosciuto.
pub fn parse_iso_utc(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let bytes = s.as_bytes();
    if bytes.len() < 10 {
        return None;
    }
    let year: i64 = s.get(0..4)?.parse().ok()?;
    let month: u32 = s.get(5..7)?.parse().ok()?;
    let day: u32 = s.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let (mut hh, mut mm, mut ss) = (0u32, 0u32, 0u32);
    if s.len() >= 13 {
        // separatore T o spazio
        let sep = bytes[10];
        if sep == b'T' || sep == b' ' {
            if let Some(v) = s.get(11..13).and_then(|v| v.parse::<u32>().ok()) {
                hh = v;
            }
            if let Some(v) = s.get(14..16).and_then(|v| v.parse::<u32>().ok()) {
                mm = v;
            }
            if let Some(v) = s.get(17..19).and_then(|v| v.parse::<u32>().ok()) {
                ss = v;
            }
        }
    }
    if hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + (hh * 3600 + mm * 60 + ss) as i64)
}

/// `14:07` in UTC.
pub fn fmt_hhmm(epoch: i64) -> String {
    let (_, _, _, h, m, _) = civil_from_unix(epoch);
    format!("{h:02}:{m:02}")
}

/// `2026-11-14 11:04:00Z`
pub fn fmt_stamp(epoch: i64) -> String {
    let (y, mo, d, h, mi, s) = civil_from_unix(epoch);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}Z")
}

/// `14/11 11:04`
pub fn fmt_short(epoch: i64) -> String {
    let (_, mo, d, h, mi, _) = civil_from_unix(epoch);
    format!("{d:02}/{mo:02} {h:02}:{mi:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_roundtrip() {
        assert_eq!(parse_iso_utc("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso_utc("2026-11-14T11:04:00Z"), Some(1_794_654_240));
        assert_eq!(parse_iso_utc("2026-11-14 05:08:00"), Some(1_794_632_880));
        assert_eq!(parse_iso_utc("2026-11-14T05:08"), Some(1_794_632_880));
        assert_eq!(parse_iso_utc("nonsense"), None);
        assert_eq!(parse_iso_utc(""), None);
    }

    #[test]
    fn civil() {
        assert_eq!(civil_from_unix(1_794_654_240), (2026, 11, 14, 11, 4, 0));
        assert_eq!(fmt_hhmm(1_794_654_240), "11:04");
    }
}
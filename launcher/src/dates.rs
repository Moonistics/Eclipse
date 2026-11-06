use super::*;

/// A release's day ("Nov 1", "Nov 1, 2025" in another year), with its time in UTC when known.
pub(super) fn published(date: &str, time: &str) -> String {
    let day = date
        .get(8..10)
        .map(|d| d.trim_start_matches('0').to_string())
        .unwrap_or_default();
    let args = [("day", day.as_str())];
    let day = match date.get(5..7) {
        Some("01") => trf("Jan {day}", &args),
        Some("02") => trf("Feb {day}", &args),
        Some("03") => trf("Mar {day}", &args),
        Some("04") => trf("Apr {day}", &args),
        Some("05") => trf("May {day}", &args),
        Some("06") => trf("Jun {day}", &args),
        Some("07") => trf("Jul {day}", &args),
        Some("08") => trf("Aug {day}", &args),
        Some("09") => trf("Sep {day}", &args),
        Some("10") => trf("Oct {day}", &args),
        Some("11") => trf("Nov {day}", &args),
        Some("12") => trf("Dec {day}", &args),
        _ => date.to_string(),
    };
    let this_year = chrono::Utc::now().format("%Y").to_string();
    let day = match date.get(..4) {
        Some(year) if year != this_year && day != date => {
            trf("{day}, {year}", &[("day", &day), ("year", year)])
        }
        _ => day,
    };
    if time.is_empty() {
        day
    } else {
        trf("{day}, {time} UTC", &[("day", &day), ("time", time)])
    }
}

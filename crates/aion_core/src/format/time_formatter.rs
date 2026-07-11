//! Time-axis label formatting. Ports of
//! `src/model/horz-scale-behavior-time/default-tick-mark-formatter.ts` (en-US behavior;
//! an `Intl`-backed locale hook comes with the JS API) and the crosshair date-time format
//! (`DateTimeFormatter` with LWC's default `dd MMM 'yy` date format).

use crate::scale::time_tick_marks::{civil_from_timestamp, TickMarkWeight};

const MONTHS_SHORT: [&str; 12] =
    ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// Port of `TickMarkType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickMarkType {
    Year,
    Month,
    DayOfMonth,
    Time,
    TimeWithSeconds,
}

/// Port of `weightToTickMarkType` (`horz-scale-behavior-time.ts`).
pub fn weight_to_tick_mark_type(weight: u8, time_visible: bool, seconds_visible: bool) -> TickMarkType {
    let w = weight;
    if w <= TickMarkWeight::Second as u8 {
        if time_visible {
            if seconds_visible {
                TickMarkType::TimeWithSeconds
            } else {
                TickMarkType::Time
            }
        } else {
            TickMarkType::DayOfMonth
        }
    } else if w < TickMarkWeight::Day as u8 {
        // minutes and hours
        if time_visible { TickMarkType::Time } else { TickMarkType::DayOfMonth }
    } else if w < TickMarkWeight::Month as u8 {
        TickMarkType::DayOfMonth
    } else if w < TickMarkWeight::Year as u8 {
        TickMarkType::Month
    } else {
        TickMarkType::Year
    }
}

fn hms(ts: i64) -> (i64, i64, i64) {
    let secs_of_day = ts.rem_euclid(86_400);
    (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60)
}

/// Tick label for a UTC timestamp — matches `defaultTickMarkFormatter` output for en-US.
pub fn format_tick_label(ts: i64, mark_type: TickMarkType) -> String {
    let (year, month, day) = civil_from_timestamp(ts);
    match mark_type {
        TickMarkType::Year => format!("{year}"),
        TickMarkType::Month => MONTHS_SHORT[(month - 1) as usize].to_string(),
        TickMarkType::DayOfMonth => format!("{day}"),
        TickMarkType::Time => {
            let (h, m, _) = hms(ts);
            format!("{h:02}:{m:02}")
        }
        TickMarkType::TimeWithSeconds => {
            let (h, m, s) = hms(ts);
            format!("{h:02}:{m:02}:{s:02}")
        }
    }
}

/// Crosshair time label: LWC's default `dd MMM 'yy` date format, plus `HH:MM` when the
/// time is visible (`DateTimeFormatter` joins them with spaces).
pub fn format_crosshair_time(ts: i64, time_visible: bool, seconds_visible: bool) -> String {
    let (year, month, day) = civil_from_timestamp(ts);
    let date = format!("{day:02} {} '{:02}", MONTHS_SHORT[(month - 1) as usize], year.rem_euclid(100));
    if !time_visible {
        return date;
    }
    let (h, m, s) = hms(ts);
    if seconds_visible {
        format!("{date}   {h:02}:{m:02}:{s:02}")
    } else {
        format!("{date}   {h:02}:{m:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2018-06-25T14:30:45Z
    const TS: i64 = 1_529_937_045;

    #[test]
    fn tick_labels() {
        assert_eq!(format_tick_label(TS, TickMarkType::Year), "2018");
        assert_eq!(format_tick_label(TS, TickMarkType::Month), "Jun");
        assert_eq!(format_tick_label(TS, TickMarkType::DayOfMonth), "25");
        assert_eq!(format_tick_label(TS, TickMarkType::Time), "14:30");
        assert_eq!(format_tick_label(TS, TickMarkType::TimeWithSeconds), "14:30:45");
    }

    #[test]
    fn weight_mapping() {
        // intraday weights show time when timeVisible
        assert_eq!(
            weight_to_tick_mark_type(TickMarkWeight::Hour1 as u8, true, false),
            TickMarkType::Time
        );
        assert_eq!(
            weight_to_tick_mark_type(TickMarkWeight::Hour1 as u8, false, false),
            TickMarkType::DayOfMonth
        );
        assert_eq!(
            weight_to_tick_mark_type(TickMarkWeight::Day as u8, true, true),
            TickMarkType::DayOfMonth
        );
        assert_eq!(
            weight_to_tick_mark_type(TickMarkWeight::Month as u8, true, true),
            TickMarkType::Month
        );
        assert_eq!(
            weight_to_tick_mark_type(TickMarkWeight::Year as u8, false, false),
            TickMarkType::Year
        );
        assert_eq!(
            weight_to_tick_mark_type(TickMarkWeight::Second as u8, true, true),
            TickMarkType::TimeWithSeconds
        );
    }

    #[test]
    fn crosshair_format() {
        assert_eq!(format_crosshair_time(TS, false, false), "25 Jun '18");
        assert_eq!(format_crosshair_time(TS, true, false), "25 Jun '18   14:30");
        assert_eq!(format_crosshair_time(TS, true, true), "25 Jun '18   14:30:45");
    }
}

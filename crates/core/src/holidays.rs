//! Holidays computed from their rules, by region. Nothing here is a table copied from elsewhere:
//! fixed dates come from the laws that set them, movable ones from Easter.

use crate::Date;

/// A region whose holidays MK Perth knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    /// Brazil's national holidays, as business days count them: the federal holidays (Lei
    /// 662/1949, 6.802/1980 for 12 October, 14.759/2023 for 20 November from 2024) and the
    /// movable days the national settlement calendar closes (Carnival Monday and Tuesday, Good
    /// Friday, Corpus Christi).
    Brazil,
}

impl Region {
    /// The region a calendar's name refers to, if MK Perth knows it.
    pub fn named(name: &str) -> Option<Region> {
        match name.to_ascii_lowercase().as_str() {
            "brazil" | "brsettlement" => Some(Region::Brazil),
            _ => None,
        }
    }

    pub fn is_holiday(self, date: Date) -> bool {
        let (year, month, day) = date.ymd();
        match self {
            Region::Brazil => {
                let fixed = matches!((month, day), (1, 1) | (4, 21) | (5, 1) | (9, 7) | (10, 12) | (11, 2) | (11, 15) | (12, 25))
                    || (year >= 2024 && (month, day) == (11, 20));
                fixed
                    || easter(year).is_some_and(|e| {
                        // Carnival Monday and Tuesday, Good Friday, Corpus Christi.
                        [-48, -47, -2, 60].iter().any(|offset| e.days().checked_add(*offset) == Some(date.days()))
                    })
            }
        }
    }
}

/// Easter Sunday of a Gregorian year, by the anonymous Gregorian algorithm (Meeus, *Astronomical
/// Algorithms*, chapter 8).
pub fn easter(year: i32) -> Option<Date> {
    let y = year;
    let a = y.rem_euclid(19);
    let b = y.div_euclid(100);
    let c = y.rem_euclid(100);
    let d = b.div_euclid(4);
    let e = b.rem_euclid(4);
    let f = (b + 8).div_euclid(25);
    let g = (b - f + 1).div_euclid(3);
    let h = (19 * a + b - d - g + 15).rem_euclid(30);
    let i = c.div_euclid(4);
    let k = c.rem_euclid(4);
    let l = (32 + 2 * e + 2 * i - h - k).rem_euclid(7);
    let m = (a + 11 * h + 22 * l).div_euclid(451);
    let month = (h + l - 7 * m + 114).div_euclid(31);
    let day = (h + l - 7 * m + 114).rem_euclid(31) + 1;
    Date::from_ymd(year, u32::try_from(month).ok()?, u32::try_from(day).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easter_sundays() {
        for (y, m, d) in [(1961, 4, 2), (2000, 4, 23), (2008, 3, 23), (2024, 3, 31), (2025, 4, 20), (2026, 4, 5), (2038, 4, 25), (2285, 3, 22)] {
            assert_eq!(easter(y), Date::from_ymd(y, m, d), "{y}");
        }
        assert!(easter(-9999).is_some() && easter(9999).is_some());
    }

    /// Brazil's holidays on weekdays, 2020–2030, from the rules above; the same list as
    /// BusinessDays.jl 0.9.25's `:Brazil` calendar gives (re-typed here as a check).
    #[test]
    fn brazil_on_weekdays_2020_to_2030() {
        let expected = [
            (2020, "01-01 02-24 02-25 04-10 04-21 05-01 06-11 09-07 10-12 11-02 12-25"),
            (2021, "01-01 02-15 02-16 04-02 04-21 06-03 09-07 10-12 11-02 11-15"),
            (2022, "02-28 03-01 04-15 04-21 06-16 09-07 10-12 11-02 11-15"),
            (2023, "02-20 02-21 04-07 04-21 05-01 06-08 09-07 10-12 11-02 11-15 12-25"),
            (2024, "01-01 02-12 02-13 03-29 05-01 05-30 11-15 11-20 12-25"),
            (2025, "01-01 03-03 03-04 04-18 04-21 05-01 06-19 11-20 12-25"),
            (2026, "01-01 02-16 02-17 04-03 04-21 05-01 06-04 09-07 10-12 11-02 11-20 12-25"),
            (2027, "01-01 02-08 02-09 03-26 04-21 05-27 09-07 10-12 11-02 11-15"),
            (2028, "02-28 02-29 04-14 04-21 05-01 06-15 09-07 10-12 11-02 11-15 11-20 12-25"),
            (2029, "01-01 02-12 02-13 03-30 05-01 05-31 09-07 10-12 11-02 11-15 11-20 12-25"),
            (2030, "01-01 03-04 03-05 04-19 05-01 06-20 11-15 11-20 12-25"),
        ];
        for (year, list) in expected {
            let mut found = Vec::new();
            let mut d = Date::from_ymd(year, 1, 1).unwrap();
            while d.ymd().0 == year {
                if d.weekday().index() < 5 && Region::Brazil.is_holiday(d) {
                    let (_, m, day) = d.ymd();
                    found.push(format!("{m:02}-{day:02}"));
                }
                d = d.succ().unwrap();
            }
            assert_eq!(found.join(" "), list, "{year}");
        }
        assert_eq!(Region::named("BRAZIL"), Some(Region::Brazil));
        assert_eq!(Region::named("Narnia"), None);
    }
}

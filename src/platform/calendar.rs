use std::ops::RangeInclusive;

use chrono::{Datelike, Local, NaiveDate};
use windows::{
    Foundation::DateTime,
    Globalization::{
        Calendar, CalendarIdentifiers, ClockIdentifiers, DateTimeFormatting::DateTimeFormatter,
    },
    Win32::Globalization::{GetLocaleInfoEx, LOCALE_IFIRSTDAYOFWEEK},
    core::{HSTRING, Result},
};
use windows_collections::IIterable;

const FILETIME_EPOCH_DAY: i32 = 584_389;
const TICKS_PER_DAY: i64 = 864_000_000_000;

/// A civil date numbered from 0001-01-01, with that day numbered 1.
pub type DateKey = i32;

pub fn today() -> DateKey {
    Local::now().date_naive().num_days_from_ce()
}

pub fn date_key(year: i32, month: u32, day: u32) -> Option<DateKey> {
    NaiveDate::from_ymd_opt(year, month, day).map(|date| date.num_days_from_ce())
}

pub fn civil_date(date: DateKey) -> Option<NaiveDate> {
    NaiveDate::from_num_days_from_ce_opt(date)
}

fn native_date(date: DateKey) -> DateTime {
    DateTime {
        UniversalTime: (i64::from(date) - i64::from(FILETIME_EPOCH_DAY)) * TICKS_PER_DAY
            + TICKS_PER_DAY / 2,
    }
}

fn key(date: DateTime) -> DateKey {
    date.UniversalTime.div_euclid(TICKS_PER_DAY) as i32 + FILETIME_EPOCH_DAY
}

#[derive(Clone, Copy)]
pub enum Unit {
    Day,
    Month,
    Year,
    Decade,
}

pub struct CalendarSystem {
    native: Calendar,
    month_year: DateTimeFormatter,
    full_date: DateTimeFormatter,
    utc: HSTRING,
    pub bounds: RangeInclusive<DateKey>,
    /// Sunday is 0.
    pub first_day_of_week: i32,
}

impl CalendarSystem {
    pub fn new(language: &str, identifier: &str) -> Result<Self> {
        let languages: IIterable<HSTRING> = if language.is_empty() {
            Calendar::new()?
                .Languages()?
                .into_iter()
                .collect::<Vec<_>>()
                .into()
        } else {
            vec![HSTRING::from(language)].into()
        };
        let identifier = if identifier.is_empty() {
            CalendarIdentifiers::Gregorian()?
        } else {
            HSTRING::from(identifier)
        };
        let clock = ClockIdentifiers::TwentyFourHour()?;
        let utc = HSTRING::from("UTC");
        let native = Calendar::CreateCalendarWithTimeZone(&languages, &identifier, &clock, &utc)?;
        let locale = native.ResolvedLanguage()?;
        // WinUI uses the neutral region so that labels follow the requested language.
        let region = HSTRING::from("ZZ");
        let formatter = |template: &str| {
            DateTimeFormatter::CreateDateTimeFormatterContext(
                &HSTRING::from(template),
                &languages,
                &region,
                &identifier,
                &clock,
            )
        };
        native.SetToMin()?;
        // Noon must still be within the calendar's supported range on the endpoint days.
        let earliest = native.GetDateTime()?.UniversalTime;
        native.SetToMax()?;
        let latest = native.GetDateTime()?.UniversalTime;
        let min = key(DateTime {
            UniversalTime: earliest + TICKS_PER_DAY / 2,
        });
        let max = key(DateTime {
            UniversalTime: latest - TICKS_PER_DAY / 2,
        });
        let mut value = [0; 2];
        let first_day_of_week =
            unsafe { GetLocaleInfoEx(&locale, LOCALE_IFIRSTDAYOFWEEK, Some(&mut value)) };
        let first_day_of_week = if first_day_of_week > 0 {
            (i32::from(value[0]) - i32::from(b'0') + 1).rem_euclid(7)
        } else {
            0
        };
        Ok(Self {
            native,
            month_year: formatter("month year")?,
            full_date: formatter("longdate")?,
            utc,
            bounds: min..=max,
            first_day_of_week,
        })
    }

    pub fn clamp(&self, date: DateKey) -> DateKey {
        date.clamp(*self.bounds.start(), *self.bounds.end())
    }

    fn position(&self, date: DateKey) -> Result<()> {
        self.native.SetDateTime(native_date(self.clamp(date)))
    }

    fn position_key(&self) -> Result<DateKey> {
        self.native
            .GetDateTime()
            .map(key)
            .map(|date| self.clamp(date))
    }

    pub fn shift(&self, date: DateKey, unit: Unit, offset: i32) -> Result<DateKey> {
        if matches!(unit, Unit::Day) {
            return Ok(self.clamp(date.saturating_add(offset)));
        }
        self.position(date)?;
        let result = match unit {
            Unit::Month => self.native.AddMonths(offset),
            Unit::Year => self.native.AddYears(offset),
            Unit::Decade => self.native.AddYears(offset.saturating_mul(10)),
            Unit::Day => unreachable!(),
        };
        if result.is_err() {
            return Ok(if offset < 0 {
                *self.bounds.start()
            } else {
                *self.bounds.end()
            });
        }
        self.position_key()
    }

    fn month_edge(&self, end: bool) -> Result<()> {
        self.native.SetDay(if end {
            self.native.LastDayInThisMonth()?
        } else {
            self.native.FirstDayInThisMonth()?
        })
    }

    fn year_edge(&self, end: bool) -> Result<()> {
        self.month_edge(false)?;
        self.native.SetMonth(if end {
            self.native.LastMonthInThisYear()?
        } else {
            self.native.FirstMonthInThisYear()?
        })?;
        self.month_edge(end)
    }

    pub fn edge(&self, date: DateKey, unit: Unit, end: bool) -> Result<DateKey> {
        self.position(date)?;
        match unit {
            Unit::Day => return Ok(self.clamp(date)),
            Unit::Month => self.month_edge(end)?,
            Unit::Year => self.year_edge(end)?,
            Unit::Decade => {
                let first = self.native.Year()?.div_euclid(10) * 10;
                let target = if end {
                    (first + 9).min(self.native.LastYearInThisEra()?)
                } else {
                    first.max(self.native.FirstYearInThisEra()?)
                };
                self.year_edge(false)?;
                self.native.SetYear(target)?;
                self.year_edge(end)?;
            }
        }
        self.position_key()
    }

    pub fn label(&self, date: DateKey, unit: Unit) -> Result<String> {
        self.position(date)?;
        let label = match unit {
            Unit::Day => self.native.DayAsString()?,
            Unit::Month => self.native.MonthAsFullSoloString()?,
            Unit::Year | Unit::Decade => self.native.YearAsString()?,
        };
        Ok(clean(label))
    }

    pub fn short_month(&self, date: DateKey) -> Result<String> {
        self.position(date)?;
        self.native.MonthAsString(3).map(clean)
    }

    pub fn year_number(&self, date: DateKey) -> Result<i32> {
        self.position(date)?;
        self.native.Year()
    }

    pub fn weekday(&self, date: DateKey) -> Result<(String, String)> {
        self.position(date)?;
        Ok((
            clean(self.native.DayOfWeekAsString(1)?),
            clean(self.native.DayOfWeekAsFullString()?),
        ))
    }

    pub fn month_year(&self, date: DateKey) -> Result<String> {
        self.month_year
            .FormatUsingTimeZone(native_date(self.clamp(date)), &self.utc)
            .map(clean)
    }

    pub fn full_date(&self, date: DateKey) -> Result<String> {
        self.full_date
            .FormatUsingTimeZone(native_date(self.clamp(date)), &self.utc)
            .map(clean)
    }

    pub fn picker_date(&self, date: DateKey, format: &str) -> Result<String> {
        let formatter = if format.is_empty() {
            DateTimeFormatter::ShortDate()?
        } else {
            self.date_formatter(format)?
        };
        formatter
            .FormatUsingTimeZone(native_date(self.clamp(date)), &self.utc)
            .map(clean)
    }

    fn date_formatter(&self, format: &str) -> Result<DateTimeFormatter> {
        DateTimeFormatter::CreateDateTimeFormatterContext(
            &HSTRING::from(format),
            &self.native.Languages()?,
            &HSTRING::from("ZZ"),
            &self.native.GetCalendarSystem()?,
            &self.native.GetClock()?,
        )
    }

    pub fn date_pattern(&self) -> Result<String> {
        self.date_formatter("day month.full year")?
            .Patterns()?
            .GetAt(0)
            .map(|value| value.to_string_lossy())
    }

    pub fn date_labels(&self, format: &str) -> Result<impl Fn(DateKey) -> String + '_> {
        let formatter = self.date_formatter(format)?;
        Ok(move |date| {
            formatter
                .FormatUsingTimeZone(native_date(self.clamp(date)), &self.utc)
                .map(clean)
                .unwrap_or_default()
        })
    }
}

fn clean(label: HSTRING) -> String {
    label
        .to_string_lossy()
        .replace(['\u{200e}', '\u{200f}'], "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gregorian_boundaries_and_leap_day() {
        let calendar = CalendarSystem::new("en-US", "GregorianCalendar").unwrap();
        let date = date_key(2028, 2, 29).unwrap();
        assert_eq!(calendar.month_year(date).unwrap(), "February 2028");
        assert_eq!(calendar.edge(date, Unit::Month, true).unwrap(), date);
        assert_eq!(
            calendar.edge(date, Unit::Year, false).unwrap(),
            date_key(2028, 1, 1).unwrap()
        );
        assert_eq!(
            calendar.shift(date, Unit::Year, 1).unwrap(),
            date_key(2029, 2, 28).unwrap()
        );
        assert_eq!(
            calendar.edge(date, Unit::Decade, false).unwrap(),
            date_key(2020, 1, 1).unwrap()
        );
        assert_eq!(calendar.first_day_of_week, 0);
    }

    #[test]
    fn calendar_systems_share_the_same_civil_day() {
        let date = date_key(2026, 11, 20).unwrap();
        assert_eq!(civil_date(date).unwrap().to_string(), "2026-11-20");
        let japanese = CalendarSystem::new("ja-JP", "JapaneseCalendar").unwrap();
        assert_eq!(japanese.year_number(date).unwrap(), 8);
        let hebrew = CalendarSystem::new("en-US", "HebrewCalendar").unwrap();
        assert_eq!(hebrew.year_number(date).unwrap(), 5787);
        let hijri = CalendarSystem::new("en-US", "HijriCalendar").unwrap();
        assert_eq!(hijri.year_number(date).unwrap(), 1448);
        for calendar in [japanese, hebrew, hijri] {
            assert_eq!(calendar.shift(date, Unit::Day, 1).unwrap(), date + 1);
            let first = calendar.edge(date, Unit::Month, false).unwrap();
            assert!((0..31).contains(&(date - first)));
            assert!(!calendar.month_year(date).unwrap().is_empty());
        }
    }
}

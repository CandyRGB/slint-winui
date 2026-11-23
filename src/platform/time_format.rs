use chrono::{Local, Timelike};
use windows::{
    Foundation::DateTime,
    Globalization::{Calendar, CalendarIdentifiers, DateTimeFormatting::DateTimeFormatter},
    core::{HSTRING, Result},
};
use windows_collections::IIterable;

pub fn local_minutes() -> i32 {
    let local = Local::now();
    (local.hour() * 60 + local.minute()) as i32
}

pub struct TimeFormat {
    full: DateTimeFormatter,
    hour: DateTimeFormatter,
    minute: DateTimeFormatter,
    period: DateTimeFormatter,
    pub pattern: String,
    pub twelve_hour: bool,
}

impl TimeFormat {
    pub fn new(language: &str, clock: &str) -> Result<Self> {
        let languages: IIterable<HSTRING> = if language.is_empty() {
            Calendar::new()?
                .Languages()?
                .into_iter()
                .collect::<Vec<_>>()
                .into()
        } else {
            vec![HSTRING::from(language)].into()
        };
        let clock = if matches!(clock, "12HourClock" | "24HourClock") {
            HSTRING::from(clock)
        } else {
            DateTimeFormatter::ShortTime()?.Clock()?
        };
        let calendar = CalendarIdentifiers::Gregorian()?;
        let region = HSTRING::from("ZZ");
        let formatter = |template: &str| {
            DateTimeFormatter::CreateDateTimeFormatterContext(
                &HSTRING::from(template),
                &languages,
                &region,
                &calendar,
                &clock,
            )
        };
        let full = formatter("hour minute")?;
        let pattern = full.Patterns()?.GetAt(0)?.to_string_lossy();
        let twelve_hour = clock == HSTRING::from("12HourClock");
        Ok(Self {
            full,
            pattern,
            twelve_hour,
            hour: formatter("{hour.integer(1)}")?,
            minute: formatter("{minute.integer(2)}")?,
            period: formatter("{period.abbreviated(2)}")?,
        })
    }

    fn format(formatter: &DateTimeFormatter, minutes: i32) -> String {
        let time = DateTime {
            UniversalTime: i64::from(minutes.rem_euclid(1440)) * 600_000_000,
        };
        formatter
            .FormatUsingTimeZone(time, &HSTRING::from("UTC"))
            .map(|text| text.to_string_lossy().replace(['\u{200e}', '\u{200f}'], ""))
            .unwrap_or_default()
    }

    pub fn full(&self, time: i32) -> String {
        Self::format(&self.full, time)
    }
    pub fn hour(&self, time: i32) -> String {
        Self::format(&self.hour, time)
    }
    pub fn minute(&self, time: i32) -> String {
        Self::format(&self.minute, time)
    }
    pub fn period(&self, time: i32) -> String {
        Self::format(&self.period, time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midnight_and_noon_use_different_periods() {
        let clock = TimeFormat::new("en-US", "12HourClock").unwrap();
        assert_eq!(clock.hour(0), "12");
        assert_eq!(clock.hour(720), "12");
        assert_eq!(clock.minute(0), "00");
        assert_eq!(clock.period(0), "AM");
        assert_eq!(clock.period(720), "PM");
        assert_ne!(clock.full(0), clock.full(720));

        let clock = TimeFormat::new("en-GB", "24HourClock").unwrap();
        assert!(!clock.twelve_hour);
        assert_eq!(clock.hour(17 * 60 + 5), "17");
        assert_eq!(clock.minute(17 * 60 + 5), "05");
    }
}

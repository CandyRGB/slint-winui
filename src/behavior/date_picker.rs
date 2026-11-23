use std::{cell::RefCell, rc::Rc};

use slint::{ModelRc, SharedString, VecModel};
use slint_winui::platform::calendar::{self, CalendarSystem, DateKey, Unit};

use crate::{
    DatePickerBehavior, DatePickerColumn, DatePickerOptions, DatePickerPart, DatePickerState,
    PickerWheelRow,
};

type CalendarCache = Option<(SharedString, SharedString, CalendarSystem)>;

fn with_calendar<T: Default>(
    cache: &RefCell<CalendarCache>,
    options: &DatePickerOptions,
    f: impl FnOnce(&CalendarSystem) -> T,
) -> T {
    let mut cache = cache.borrow_mut();
    if !cache.as_ref().is_some_and(|(lang, id, _)| {
        *lang == options.language && *id == options.calendar_identifier
    }) {
        *cache = CalendarSystem::new(&options.language, &options.calendar_identifier)
            .or_else(|_| CalendarSystem::new("", ""))
            .ok()
            .map(|system| {
                (
                    options.language.clone(),
                    options.calendar_identifier.clone(),
                    system,
                )
            });
    }
    cache
        .as_ref()
        .map_or_else(T::default, |(_, _, system)| f(system))
}

pub fn install(behavior: DatePickerBehavior<'_>) {
    let cache = Rc::new(RefCell::new(None));
    let state_cache = cache.clone();
    behavior.on_state(move |options| {
        with_calendar(&state_cache, &options, |system| state(system, &options))
    });
    let step_cache = cache.clone();
    behavior.on_step(move |options, part, amount| {
        with_calendar(&step_cache, &options, |system| {
            let Some(range) = range(system, &options) else {
                return options.date;
            };
            let value = range.displayed(options.date);
            stepped(system, &range, value, part, amount).unwrap_or(value)
        })
    });
    behavior.on_value_text(move |options| {
        with_calendar(&cache, &options, |system| {
            if options.date == 0 {
                return SharedString::default();
            }
            range(system, &options)
                .and_then(|range| {
                    system
                        .picker_date(range.displayed(options.date), "shortdate")
                        .ok()
                })
                .unwrap_or_default()
                .into()
        })
    });
    behavior.set_ready(true);
}

struct DateRange {
    today: DateKey,
    min: DateKey,
    max: DateKey,
}

impl DateRange {
    fn displayed(&self, date: DateKey) -> DateKey {
        (if date == 0 { self.today } else { date }).clamp(self.min, self.max)
    }
}

fn range(system: &CalendarSystem, options: &DatePickerOptions) -> Option<DateRange> {
    let today = if options.today == 0 {
        calendar::today()
    } else {
        options.today
    };
    let min_year = if options.min_year == 0 {
        system.shift(today, Unit::Year, -100).ok()?
    } else {
        options.min_year
    };
    let max_year = if options.max_year == 0 {
        system.shift(today, Unit::Year, 100).ok()?
    } else {
        options.max_year
    };
    let min = system.edge(min_year, Unit::Year, false).ok()?;
    let max = system.edge(max_year, Unit::Year, true).ok()?;
    (min <= max).then_some(DateRange { today, min, max })
}

fn stepped(
    system: &CalendarSystem,
    range: &DateRange,
    date: DateKey,
    part: DatePickerPart,
    amount: i32,
) -> Option<DateKey> {
    let target = match part {
        DatePickerPart::Day => {
            let first = system.edge(date, Unit::Month, false).ok()?;
            let last = system.edge(date, Unit::Month, true).ok()?;
            first + (date - first + amount).rem_euclid(last - first + 1)
        }
        DatePickerPart::Month => {
            let first = system.edge(date, Unit::Year, false).ok()?;
            let last = system.edge(date, Unit::Year, true).ok()?;
            let mut cursor = first;
            let mut index = 0;
            let mut count = 0;
            while cursor <= last {
                let end = system.edge(cursor, Unit::Month, true).ok()?;
                if end < cursor {
                    return None;
                }
                if (cursor..=end).contains(&date) {
                    index = count;
                }
                count += 1;
                cursor = end.checked_add(1)?;
            }
            let next = (index + amount).rem_euclid(count);
            system.shift(date, Unit::Month, next - index).ok()?
        }
        DatePickerPart::Year => system.shift(date, Unit::Year, amount).ok()?,
    };
    (range.min..=range.max).contains(&target).then_some(target)
}

fn state(system: &CalendarSystem, options: &DatePickerOptions) -> DatePickerState {
    let Some(range) = range(system, options) else {
        return DatePickerState {
            date: options.date,
            ..Default::default()
        };
    };
    let initial = range.displayed(options.date);
    let pattern = system
        .date_pattern()
        .unwrap_or_else(|_| "{month} {day} {year}".into());
    let rtl = options.rtl || pattern.starts_with('\u{200f}');
    let parts = [
        (
            DatePickerPart::Month,
            "{month",
            options.month_visible,
            options.month_format.as_str(),
            "{month.full}",
        ),
        (
            DatePickerPart::Day,
            "{day",
            options.day_visible,
            options.day_format.as_str(),
            "{day.integer}",
        ),
        (
            DatePickerPart::Year,
            "{year",
            options.year_visible,
            options.year_format.as_str(),
            "{year.full}",
        ),
    ];
    let mut columns = Vec::new();
    for (part, token, visible, format, fallback) in parts {
        if !visible {
            continue;
        }
        let format = system
            .date_labels(format)
            .or_else(|_| system.date_labels(fallback));
        let label = |date| {
            format
                .as_ref()
                .map(|formatter| formatter(date))
                .unwrap_or_default()
        };
        let rows = (-4..=4)
            .filter_map(|offset| {
                let date = stepped(system, &range, initial, part, offset)?;
                Some(PickerWheelRow {
                    text: label(date).into(),
                    offset,
                })
            })
            .collect::<Vec<_>>();
        columns.push((
            pattern.find(token).unwrap_or(usize::MAX),
            DatePickerColumn {
                part,
                text: label(initial).into(),
                rows: ModelRc::new(VecModel::from(rows)),
            },
        ));
    }
    columns.sort_by_key(|(index, _)| *index);
    if rtl {
        columns.reverse();
    }
    DatePickerState {
        date: if options.date == 0 { 0 } else { initial },
        initial,
        rtl,
        columns: ModelRc::new(VecModel::from(
            columns
                .into_iter()
                .map(|(_, column)| column)
                .collect::<Vec<_>>(),
        )),
    }
}

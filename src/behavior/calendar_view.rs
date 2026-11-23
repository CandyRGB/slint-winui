use std::{cell::RefCell, collections::HashSet, rc::Rc};

use slint::{Model, ModelRc, SharedString, VecModel};
use slint_winui::platform::calendar::{self, CalendarSystem, DateKey, Unit};

use crate::{
    CalendarDateText, CalendarViewBehavior, CalendarViewCell, CalendarViewDayName,
    CalendarViewDisplayMode, CalendarViewOptions, CalendarViewPage, CalendarViewSelectionChange,
    CalendarViewSelectionMode,
};

#[derive(Default)]
struct CalendarCache {
    locale: (SharedString, SharedString),
    system: Option<CalendarSystem>,
}

impl CalendarCache {
    fn get(&mut self, options: &CalendarViewOptions) -> Option<&CalendarSystem> {
        if self.system.is_none()
            || self.locale.0 != options.language
            || self.locale.1 != options.calendar_identifier
        {
            self.system = CalendarSystem::new(&options.language, &options.calendar_identifier)
                .or_else(|_| CalendarSystem::new("", ""))
                .ok();
            self.locale = (
                options.language.clone(),
                options.calendar_identifier.clone(),
            );
        }
        self.system.as_ref()
    }
}

pub fn install(behavior: CalendarViewBehavior<'_>) {
    let system = Rc::new(RefCell::new(CalendarCache::default()));
    let for_page = system.clone();
    behavior.on_page(move |options| {
        for_page
            .borrow_mut()
            .get(&options)
            .map(|system| page(system, &options))
            .unwrap_or_default()
    });
    let for_text = system.clone();
    behavior.on_date_text(move |options, format| {
        if options.date == 0 {
            return CalendarDateText::default();
        }
        let mut system = for_text.borrow_mut();
        let Some(calendar) = system.get(&options) else {
            return CalendarDateText {
                date: options.date,
                text: SharedString::default(),
            };
        };
        let date = limits(calendar, &options).date;
        let text = calendar
            .picker_date(date, &format)
            .or_else(|_| calendar.picker_date(date, ""))
            .unwrap_or_default();
        CalendarDateText {
            date,
            text: text.into(),
        }
    });
    behavior.on_date_step(move |options, direction, across_scope| {
        let mut system = system.borrow_mut();
        let Some(system) = system.get(&options) else {
            return options.date;
        };
        let limits = limits(system, &options);
        let unit = if across_scope {
            scope(options.mode)
        } else {
            item_unit(options.mode)
        };
        system
            .shift(limits.date, unit, direction)
            .unwrap_or(limits.date)
            .clamp(limits.min, limits.max)
    });
    behavior.on_selection(update_selection);
    behavior.on_selection_change(selection_change);
    behavior.set_ready(true);
}

fn scope(mode: CalendarViewDisplayMode) -> Unit {
    match mode {
        CalendarViewDisplayMode::Month => Unit::Month,
        CalendarViewDisplayMode::Year => Unit::Year,
        CalendarViewDisplayMode::Decade => Unit::Decade,
    }
}

fn item_unit(mode: CalendarViewDisplayMode) -> Unit {
    match mode {
        CalendarViewDisplayMode::Month => Unit::Day,
        CalendarViewDisplayMode::Year => Unit::Month,
        CalendarViewDisplayMode::Decade => Unit::Year,
    }
}

struct Limits {
    today: DateKey,
    date: DateKey,
    min: DateKey,
    max: DateKey,
}

fn limits(system: &CalendarSystem, options: &CalendarViewOptions) -> Limits {
    let today = if options.today == 0 {
        calendar::today()
    } else {
        options.today
    };
    let min = if options.min_date == 0 {
        system
            .shift(today, Unit::Year, -100)
            .unwrap_or(*system.bounds.start())
    } else {
        system.clamp(options.min_date)
    };
    let max = if options.max_date == 0 {
        system
            .shift(today, Unit::Year, 100)
            .unwrap_or(*system.bounds.end())
    } else {
        system.clamp(options.max_date)
    }
    .max(min);
    let date = if options.date == 0 {
        today
    } else {
        options.date
    }
    .clamp(min, max);
    Limits {
        today,
        date,
        min,
        max,
    }
}

fn weekday_of(date: DateKey) -> i32 {
    date.rem_euclid(7)
}

fn page(system: &CalendarSystem, options: &CalendarViewOptions) -> CalendarViewPage {
    let limits = limits(system, options);
    let mode = options.mode;
    let unit = item_unit(mode);
    let first = system
        .edge(limits.date, scope(mode), false)
        .unwrap_or(limits.date);
    let last = system
        .edge(limits.date, scope(mode), true)
        .unwrap_or(limits.date);
    let column_count = options.columns.clamp(2, 8);
    let count = column_count * options.rows.clamp(2, 8);
    let week_start = if options.first_day_of_week < 0 {
        system.first_day_of_week
    } else {
        options.first_day_of_week.rem_euclid(7)
    };

    let title = match mode {
        CalendarViewDisplayMode::Month => system.month_year(limits.date).unwrap_or_default(),
        CalendarViewDisplayMode::Year => system.label(limits.date, Unit::Year).unwrap_or_default(),
        CalendarViewDisplayMode::Decade => format!(
            "{} - {}",
            system.label(first, Unit::Year).unwrap_or_default(),
            system.label(last, Unit::Year).unwrap_or_default()
        ),
    };

    let start = match mode {
        CalendarViewDisplayMode::Month => {
            let aligned = first - (weekday_of(first) - week_start).rem_euclid(7);
            // Keep the focused week visible when the viewport has fewer than six rows.
            let overflow = (limits.date - aligned).div_euclid(7) + 1 - count / 7;
            aligned + 7 * overflow.max(0)
        }
        CalendarViewDisplayMode::Year => first,
        CalendarViewDisplayMode::Decade => {
            let first_year = system.year_number(first).unwrap_or(0);
            let min_year = system.year_number(limits.min).unwrap_or(first_year);
            let preceding = (first_year - min_year).rem_euclid(column_count);
            system.shift(first, Unit::Year, -preceding).unwrap_or(first)
        }
    };
    let start = if mode == CalendarViewDisplayMode::Month {
        start
    } else {
        fit_scope(system, limits.date, start, unit, column_count, count)
    };
    let weekdays = if mode == CalendarViewDisplayMode::Month {
        (0..7)
            .map(|i| {
                let (text, name) = system.weekday(start + i).unwrap_or_default();
                CalendarViewDayName {
                    text: text.into(),
                    name: name.into(),
                }
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let mut focused = -1;
    let mut cells = Vec::with_capacity(count as usize);
    let mut cursor = start;
    for index in 0..count {
        let Some(cell) = make_cell(system, &limits, mode, unit, cursor, first, last) else {
            cells.push(CalendarViewCell::default());
            cursor = if matches!(unit, Unit::Day) {
                cursor + 1
            } else {
                i32::MAX
            };
            continue;
        };
        let end = system.edge(cursor, unit, true).unwrap_or(cursor);
        if (cursor..=end).contains(&limits.date) {
            focused = index;
        }
        cells.push(cell);
        cursor = match unit {
            Unit::Day => cursor + 1,
            _ => end.saturating_add(1),
        };
    }
    CalendarViewPage {
        title: title.into(),
        date: limits.date,
        first: first.clamp(limits.min, limits.max),
        last: last.clamp(limits.min, limits.max),
        minimum: limits.min,
        maximum: limits.max,
        focused,
        previous: first > limits.min,
        next: last < limits.max,
        weekdays: ModelRc::new(VecModel::from(weekdays)),
        cells: ModelRc::new(VecModel::from(cells)),
    }
}

fn make_cell(
    system: &CalendarSystem,
    limits: &Limits,
    mode: CalendarViewDisplayMode,
    unit: Unit,
    start: DateKey,
    first: DateKey,
    last: DateKey,
) -> Option<CalendarViewCell> {
    if !system.bounds.contains(&start) {
        return None;
    }
    let end = system.edge(start, unit, true).ok()?;
    let available = end >= limits.min && start <= limits.max;
    let today = (start..=end).contains(&limits.today);
    let text = if mode == CalendarViewDisplayMode::Year {
        system.short_month(start).ok()?
    } else {
        system.label(start, unit).ok()?
    };
    let name = if mode == CalendarViewDisplayMode::Month {
        system.full_date(start).unwrap_or_else(|_| text.clone())
    } else if mode == CalendarViewDisplayMode::Year {
        system.label(start, unit).unwrap_or_else(|_| text.clone())
    } else {
        text.clone()
    };
    let group = match mode {
        CalendarViewDisplayMode::Month
            if system.edge(start, Unit::Month, false).ok() == Some(start) =>
        {
            system.short_month(start).unwrap_or_default()
        }
        CalendarViewDisplayMode::Year
            if system.edge(start, Unit::Year, false).ok() == Some(start) =>
        {
            system.label(start, Unit::Year).unwrap_or_default()
        }
        CalendarViewDisplayMode::Decade => String::new(),
        _ => String::new(),
    };
    Some(CalendarViewCell {
        date: if available {
            start.clamp(limits.min, limits.max)
        } else {
            start
        },
        text: text.into(),
        name: name.into(),
        group: group.into(),
        available,
        today,
        in_scope: (first..=last).contains(&start),
    })
}

fn fit_scope(
    system: &CalendarSystem,
    active: DateKey,
    start: DateKey,
    unit: Unit,
    cols: i32,
    count: i32,
) -> DateKey {
    let mut starts = vec![start];
    let mut cursor = start;
    // At most a calendar year of months, or a decade and one incomplete row of years.
    while let Ok(end) = system.edge(cursor, unit, true) {
        if end >= active || end < cursor || end >= *system.bounds.end() {
            break;
        }
        cursor = end + 1;
        starts.push(cursor);
    }
    let offset = (starts.len() as i32 - 1 + cols - count)
        .div_euclid(cols)
        .max(0)
        * cols;
    starts[offset as usize]
}

fn update_selection(
    current: ModelRc<DateKey>,
    blackout: ModelRc<DateKey>,
    mode: CalendarViewSelectionMode,
    date: DateKey,
    min: DateKey,
    max: DateKey,
) -> ModelRc<DateKey> {
    let excluded: HashSet<_> = blackout.iter().collect();
    let mut seen = HashSet::new();
    let eligible =
        |date: &DateKey| (min..=max).contains(date) && !excluded.contains(date) && *date != 0;
    let mut next = current
        .iter()
        .filter(|day| eligible(day) && seen.insert(*day))
        .take(match mode {
            CalendarViewSelectionMode::None => 0,
            CalendarViewSelectionMode::Single => 1,
            CalendarViewSelectionMode::Multiple => usize::MAX,
        })
        .collect::<Vec<_>>();
    if mode != CalendarViewSelectionMode::None && eligible(&date) {
        if let Some(index) = next.iter().position(|selected| *selected == date) {
            next.remove(index);
        } else {
            if mode == CalendarViewSelectionMode::Single {
                next.clear();
            }
            next.push(date);
        }
    }
    if current.iter().eq(next.iter().copied()) {
        current
    } else {
        ModelRc::new(VecModel::from(next))
    }
}

fn selection_change(
    before: ModelRc<DateKey>,
    after: ModelRc<DateKey>,
) -> CalendarViewSelectionChange {
    let old = before.iter().collect::<HashSet<_>>();
    let new = after.iter().collect::<HashSet<_>>();
    CalendarViewSelectionChange {
        added: ModelRc::new(VecModel::from(
            after
                .iter()
                .filter(|date| !old.contains(date))
                .collect::<Vec<_>>(),
        )),
        removed: ModelRc::new(VecModel::from(
            before
                .iter()
                .filter(|date| !new.contains(date))
                .collect::<Vec<_>>(),
        )),
    }
}

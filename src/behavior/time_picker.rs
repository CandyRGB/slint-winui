use std::cell::RefCell;

use slint::{ModelRc, SharedString, VecModel};
use slint_winui::platform::time_format::{TimeFormat, local_minutes};

use crate::{
    PickerWheelRow, TimePickerBehavior, TimePickerColumn, TimePickerOptions, TimePickerPart,
    TimePickerState,
};

pub fn install(behavior: TimePickerBehavior<'_>) {
    let clock = RefCell::new(None::<(SharedString, SharedString, TimeFormat)>);
    behavior.on_state(move |options| {
        let mut current = clock.borrow_mut();
        if !current
            .as_ref()
            .is_some_and(|(lang, clock, _)| *lang == options.language && *clock == options.clock)
        {
            *current = TimeFormat::new(&options.language, &options.clock)
                .or_else(|_| TimeFormat::new("", &options.clock))
                .ok()
                .map(|clock| (options.language.clone(), options.clock.clone(), clock));
        }
        current.as_ref().map_or_else(
            || TimePickerState {
                time: options.time,
                ..Default::default()
            },
            |(_, _, clock)| state(clock, &options),
        )
    });
    behavior.on_step(step);
    behavior.set_ready(true);
}

fn increment(value: i32) -> i32 {
    // WinUI treats zero as an hour-wide step, so only :00 is available.
    if value == 0 { 60 } else { value.clamp(1, 59) }
}

fn round_time(time: i32, minutes: i32) -> i32 {
    let time = time.rem_euclid(1440);
    time - time % 60 % minutes
}

fn step(
    time: i32,
    part: TimePickerPart,
    amount: i32,
    minute_increment: i32,
    twelve_hour: bool,
) -> i32 {
    let minutes = increment(minute_increment);
    let time = round_time(time, minutes);
    let hour = time / 60;
    let minute = time % 60;
    match part {
        TimePickerPart::Hour => {
            let count = if twelve_hour { 12 } else { 24 };
            let hour = hour / count * count + (hour % count + amount.rem_euclid(count)) % count;
            hour * 60 + minute
        }
        TimePickerPart::Minute => {
            let count = (59 / minutes) + 1;
            let minute = (minute / minutes + amount.rem_euclid(count)) % count * minutes;
            hour * 60 + minute
        }
        TimePickerPart::Period => {
            let period = (hour / 12).saturating_add(amount).clamp(0, 1);
            (hour % 12 + period * 12) * 60 + minute
        }
    }
}

fn state(clock: &TimeFormat, options: &TimePickerOptions) -> TimePickerState {
    let unset = options.time < 0;
    let initial = round_time(
        if !unset {
            options.time
        } else if options.now >= 0 {
            options.now
        } else {
            local_minutes()
        },
        increment(options.minute_increment),
    );
    let rtl = options.rtl || clock.pattern.starts_with('\u{200f}');
    let mut columns = Vec::new();
    let level = initial / 720;
    for (part, token) in [
        (TimePickerPart::Hour, "{hour"),
        (TimePickerPart::Minute, "{minute"),
        (TimePickerPart::Period, "{period"),
    ] {
        let offsets = match part {
            TimePickerPart::Period if !clock.twelve_hour => continue,
            TimePickerPart::Period => -level..=1 - level,
            TimePickerPart::Minute if options.minute_increment == 0 => 0..=0,
            _ => -4..=4,
        };
        let label = |time| match part {
            TimePickerPart::Hour => clock.hour(time),
            TimePickerPart::Minute => clock.minute(time),
            TimePickerPart::Period => clock.period(time),
        };
        let rows = offsets
            .map(|offset| PickerWheelRow {
                text: label(step(
                    initial,
                    part,
                    offset,
                    options.minute_increment,
                    clock.twelve_hour,
                ))
                .into(),
                offset,
            })
            .collect::<Vec<_>>();
        let display = if unset && part == TimePickerPart::Period {
            0
        } else {
            initial
        };
        columns.push((
            clock.pattern.find(token).unwrap_or(usize::MAX),
            TimePickerColumn {
                part,
                text: label(display).into(),
                rows: ModelRc::new(VecModel::from(rows)),
            },
        ));
    }
    columns.sort_by_key(|(order, _)| *order);
    if rtl {
        columns.reverse();
    }
    TimePickerState {
        time: if unset { -1 } else { initial },
        initial,
        twelve_hour: clock.twelve_hour,
        rtl,
        value_text: if unset {
            SharedString::default()
        } else {
            clock.full(initial).into()
        },
        columns: ModelRc::new(VecModel::from(
            columns
                .into_iter()
                .map(|(_, column)| column)
                .collect::<Vec<_>>(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_round_down_and_wrap_within_hour() {
        assert_eq!(round_time(17 * 60 + 59, increment(15)), 17 * 60 + 45);
        assert_eq!(round_time(17 * 60 + 59, increment(0)), 17 * 60);
        assert_eq!(
            step(17 * 60 + 51, TimePickerPart::Minute, 1, 17, false),
            17 * 60
        );
    }

    #[test]
    fn hour_wrap_and_period_are_independent() {
        assert_eq!(step(23 * 60, TimePickerPart::Hour, 1, 1, false), 0);
        assert_eq!(step(23 * 60, TimePickerPart::Hour, 1, 1, true), 720);
        assert_eq!(step(0, TimePickerPart::Period, 1, 1, true), 720);
        assert_eq!(step(720, TimePickerPart::Period, 1, 1, true), 720);
    }
}

use slint::{ModelRc, Timer, VecModel};
use std::time::Duration;

slint::slint! {
    import { AutoSuggestBoxExample, AutoSuggestion } from "ui/auto_suggest_box.slint";
    export { AutoSuggestBoxExample, AutoSuggestion }
}

const CITIES: &[(&str, &str, &str)] = &[
    ("rome", "Rome, Italy", "Rome"),
    ("rochester", "Rochester, United States", "Rochester"),
    ("rotterdam", "Rotterdam, Netherlands", "Rotterdam"),
    ("berlin", "Berlin, Germany", "Berlin"),
    ("bergen", "Bergen, Norway", "Bergen"),
    ("bern", "Bern, Switzerland", "Bern"),
    ("san-diego", "San Diego, United States", "San Diego"),
    (
        "san-francisco",
        "San Francisco, United States",
        "San Francisco",
    ),
    ("san-jose", "San José, Costa Rica", "San José"),
    ("san-juan", "San Juan, Puerto Rico", "San Juan"),
    ("san-sebastian", "San Sebastián, Spain", "San Sebastián"),
    ("santa-fe", "Santa Fe, United States", "Santa Fe"),
    ("santa-cruz", "Santa Cruz, Bolivia", "Santa Cruz"),
    ("santander", "Santander, Spain", "Santander"),
];

fn main() -> Result<(), slint::PlatformError> {
    let window = AutoSuggestBoxExample::new()?;
    let args: Vec<_> = std::env::args().skip(1).collect();
    window.set_dark(args.iter().any(|argument| argument == "--dark"));
    window.set_above(args.iter().any(|argument| argument == "--above"));

    let weak = window.as_weak();
    window.on_suggestions_requested(move |query, revision| {
        let weak = weak.clone();
        Timer::single_shot(Duration::from_millis(90), move || {
            let Some(window) = weak
                .upgrade()
                .filter(|window| window.invoke_is_current(revision))
            else {
                return;
            };
            let query = query.trim().to_lowercase();
            let rows: Vec<_> = CITIES
                .iter()
                .filter(|(_, _, city)| !query.is_empty() && city.to_lowercase().starts_with(&query))
                .map(|(id, label, city)| AutoSuggestion {
                    id: (*id).into(),
                    label: (*label).into(),
                    suggested_text: (*city).into(),
                })
                .collect();
            window.set_cities(ModelRc::new(VecModel::from(rows)));
        });
    });
    window.run()
}

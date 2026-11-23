use slint::SharedString;

pub fn limit_edit(
    previous: &str,
    draft: SharedString,
    cursor: i32,
    maximum: i32,
) -> (SharedString, i32) {
    let cursor = draft.floor_char_boundary((cursor.max(0) as usize).min(draft.len()));
    if maximum <= 0 {
        return (draft, cursor as i32);
    }

    // A programmatic value can be longer than MaxLength. Allow subsequent edits to shorten it.
    let budget = (maximum as usize).max(previous.encode_utf16().count());
    if draft.encode_utf16().count() <= budget {
        return (draft, cursor as i32);
    }

    let (head, tail) = draft.split_at(cursor);
    let unchanged_head = previous.strip_suffix(tail).unwrap_or(previous);
    let prefix_bytes: usize = head
        .chars()
        .zip(unchanged_head.chars())
        .take_while(|(current, old)| current == old)
        .map(|(ch, _)| ch.len_utf8())
        .sum();
    let (prefix, inserted) = head.split_at(prefix_bytes);
    let mut remaining =
        budget.saturating_sub(prefix.encode_utf16().count() + tail.encode_utf16().count());
    let inserted_bytes: usize = inserted
        .chars()
        .take_while(|ch| {
            if let Some(left) = remaining.checked_sub(ch.len_utf16()) {
                remaining = left;
                true
            } else {
                false
            }
        })
        .map(char::len_utf8)
        .sum();
    let end = prefix_bytes + inserted_bytes;
    (format!("{}{tail}", &head[..end]).into(), end as i32)
}

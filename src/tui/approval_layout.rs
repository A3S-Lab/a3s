#[cfg(test)]
fn approval_menu_lines(label: &str, selected: usize, width: usize) -> Vec<String> {
    approval_prompt(label, selected).lines(width)
}

const FULLSCREEN_APPROVAL_ROWS_BELOW: usize = 1;

fn approval_rows_below_for(transcript_open: bool, composer_rows_below: usize) -> usize {
    if transcript_open {
        FULLSCREEN_APPROVAL_ROWS_BELOW
    } else {
        composer_rows_below
    }
}

fn approval_prompt(label: &str, selected: usize) -> ApprovalPrompt {
    ApprovalPrompt::new(label, selected)
}

fn approval_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

#[cfg(test)]
mod tests;

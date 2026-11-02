use super::DetailState;

#[derive(Default)]
pub struct DetailViewport {
    pub pending_scroll: i32,
    pub scroll: usize,
    pub height: usize,
    pub positioned_section: Option<usize>,
}

impl DetailViewport {
    pub fn scroll_by(&mut self, delta: i32) {
        self.pending_scroll = self.pending_scroll.saturating_add(delta);
    }

    pub(super) fn section_changed(&mut self) {
        self.pending_scroll = 0;
        self.positioned_section = None;
    }

    pub fn place_section(
        &mut self,
        section: usize,
        context_rows: std::ops::Range<usize>,
        total_rows: usize,
        focused_row: Option<usize>,
        resized: bool,
    ) {
        let mut scroll = self.scroll;
        if self.positioned_section != Some(section) || resized {
            if section == 0 {
                scroll = 0;
            } else if context_rows.start < scroll
                || context_rows.end.saturating_sub(context_rows.start) >= self.height
            {
                scroll = context_rows.start;
            } else if context_rows.end > scroll.saturating_add(self.height) {
                scroll = context_rows.end.saturating_sub(self.height);
            }
        }
        if let Some(row) = focused_row {
            if row < scroll {
                scroll = row;
            } else if row >= scroll.saturating_add(self.height) {
                scroll = row.saturating_sub(self.height.saturating_sub(1));
            }
        }
        self.scroll = scroll
            .saturating_add_signed(self.pending_scroll as isize)
            .min(total_rows.saturating_sub(self.height));
        self.pending_scroll = 0;
        self.positioned_section = Some(section);
    }
}

impl DetailState {
    pub fn place_fullscreen(&mut self, height: usize, rebuilt: bool, resized: bool) {
        self.viewport.height = height;
        let Some(cache) = &self.fullscreen_cache else {
            return;
        };
        let section = self.section;
        let total_rows = cache.lines.len();
        let section_line = cache.comment_offsets.get(section).copied().unwrap_or(0);
        let section_end = cache
            .section_offsets
            .iter()
            .skip(section + 1)
            .copied()
            .find(|&offset| offset > section_line)
            .unwrap_or(total_rows);
        // Keep the code excerpt beside its reply when both fit, so the reply has context.
        let navigation_line = if self.reader.is_some() && cache.focused_row.is_none() {
            // Start at the diff when opening a comment so its code context stays visible.
            cache
                .section_offsets
                .get(section)
                .copied()
                .unwrap_or(section_line)
        } else {
            self.snapshot
                .as_ref()
                .and_then(|detail| {
                    section
                        .checked_sub(1)
                        .filter(|&i| i < detail.comments.len())
                })
                .and_then(|index| Some(self.review_threads()?.thread_start(index) + 1))
                .and_then(|start| cache.section_offsets.get(start).copied())
                .filter(|start| section_end.saturating_sub(*start) <= height)
                .unwrap_or(section_line)
        };
        let focused_row = cache.focused_row.filter(|_| rebuilt);
        self.viewport.place_section(
            section,
            navigation_line..section_end,
            total_rows,
            focused_row,
            resized,
        );
    }
}

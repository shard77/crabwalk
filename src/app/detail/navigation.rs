use crate::app::reader::ReaderAction;
use crate::app::{App, ReaderMode};
use crate::markdown::{CODE_PREVIEW_LINES, InteractiveKind};

impl App {
    fn is_section_expanded(&self) -> bool {
        self.main
            .detail
            .expansion
            .is_section_expanded(self.main.detail.section, self.main.detail.snapshot())
    }

    fn focus_adjacent_section(&mut self, backwards: bool) {
        let sections = self.detail_sections();
        let index = sections
            .iter()
            .position(|&section| section == self.main.detail.section)
            .unwrap_or(0);
        let target = if backwards {
            index.saturating_sub(1)
        } else {
            (index + 1).min(sections.len() - 1)
        };
        if target != index {
            self.set_detail_section(sections[target]);
        }
    }

    pub fn navigate_detail_code_block(&mut self, forward: bool) -> bool {
        let Some(reader) = self.main.detail.reader.as_ref() else {
            return forward && self.focus_detail_code_block();
        };
        if reader.mode != ReaderMode::Normal || !reader.inline_code {
            return false;
        }
        let Some(index) = reader.focused_element else {
            return false;
        };
        let InteractiveKind::CodeBlock { id, line_count, .. } =
            reader.rendered.interactive[index].kind
        else {
            return false;
        };
        if line_count > CODE_PREVIEW_LINES && reader.expanded_code_blocks.contains(&id) {
            return false;
        }
        let blocks = &reader.rendered.interactive;
        let target = if forward {
            (index + 1..blocks.len())
                .find(|&i| matches!(blocks[i].kind, InteractiveKind::CodeBlock { .. }))
        } else {
            (0..index)
                .rev()
                .find(|&i| matches!(blocks[i].kind, InteractiveKind::CodeBlock { .. }))
        };
        if let Some(target) = target {
            let reader = self.main.detail.reader.as_mut().unwrap();
            reader.focused_element = Some(target);
            reader.cursor = (reader.rendered.interactive[target].line_start, 0);
            reader.pending_prefix = None;
            self.main.detail.fullscreen_cache = None;
        } else {
            let next = if forward {
                let Some(next) = self
                    .detail_sections()
                    .into_iter()
                    .find(|&section| section > self.main.detail.section)
                else {
                    return true;
                };
                next
            } else {
                self.main.detail.section
            };
            self.unfocus_detail();
            self.set_detail_section(next);
        }
        true
    }

    fn detail_sections(&self) -> Vec<usize> {
        self.main
            .detail
            .snapshot()
            .as_ref()
            .map(|detail| {
                let visibility = self
                    .main
                    .detail
                    .review_threads()
                    .expect("detail present")
                    .visibility_for(&self.main.detail.expansion.expanded_comments(detail));
                std::iter::once(0)
                    .chain(
                        (0..detail.comments.len())
                            .filter(|&index| visibility.representative(index) == index)
                            .map(|index| index + 1),
                    )
                    .collect()
            })
            .unwrap_or_else(|| vec![0])
    }

    pub fn scroll_detail(&mut self, delta: i32) {
        self.main.detail.viewport.scroll_by(delta);
        if let Some(reader) = &mut self.main.detail.reader {
            reader.pending_prefix = None;
        }
    }

    pub fn move_detail_section(&mut self, delta: isize) {
        let sections = self.detail_sections();
        let index = sections
            .iter()
            .position(|&section| section == self.main.detail.section)
            .unwrap_or(0);
        let target = index.saturating_add_signed(delta).min(sections.len() - 1);
        self.unfocus_detail();
        self.set_detail_section(sections[target]);
    }

    pub fn start_detail_search(&mut self) {
        if self.main.detail.reader.is_none() {
            self.enter_reader();
        }
        if let Some(reader) = &mut self.main.detail.reader {
            reader.start_search();
        }
        self.main.detail.fullscreen_cache = None;
    }

    pub fn move_detail_focus(&mut self, backwards: bool) {
        let at_boundary = self.main.detail.reader.as_ref().is_some_and(|reader| {
            if backwards {
                reader.focused_element.is_none_or(|index| index == 0)
            } else {
                reader
                    .focused_element
                    .is_none_or(|index| index + 1 >= reader.rendered.interactive.len())
            }
        });
        if at_boundary {
            self.unfocus_detail();
            if !backwards {
                self.focus_adjacent_section(false);
            }
        } else if self.main.detail.reader.is_some() {
            self.apply_reader_action(if backwards {
                ReaderAction::Up
            } else {
                ReaderAction::Down
            });
        } else {
            self.focus_adjacent_section(backwards);
        }
    }

    pub fn activate_detail_section(&mut self, toggle: bool) {
        if toggle && self.is_section_expanded() {
            self.collapse_detail_section();
        } else {
            self.enter_reader();
        }
    }
}

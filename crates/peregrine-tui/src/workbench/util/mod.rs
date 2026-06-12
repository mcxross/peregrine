mod cache;
mod path;
mod rect;
mod text;

pub(crate) use cache::bytecode_cache_stamp;
pub(crate) use path::{nearest_move_package_root, normalized_path_string, relative_path_label};
pub(crate) use rect::{centered_rect, inner_rect, rect_contains, usize_to_u16_saturating};
pub(crate) use text::{
    char_len, char_to_byte_index, editable_char_modifiers, split_lines, styled_text_segments,
};

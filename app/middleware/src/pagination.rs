#[derive(Debug)]
pub(crate) struct Pagination {
    pub(crate) has_next_page: bool,
    pub(crate) end_cursor: Option<String>,
}

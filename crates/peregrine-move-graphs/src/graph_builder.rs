mod builder;
mod source_spans;
mod summaries;

#[cfg(test)]
mod tests;

pub(crate) use builder::{
    MoveStateAccessGraphTarget, build_move_graphs, build_move_state_access_graph,
};
